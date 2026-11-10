//! Source player headers and their local PlayerEvents result word.
//!
//! The remc2 `dw_w_b.word[1]` result is a 16-bit field. Source action 29
//! reaches PlayerEvents case 0x1D, which writes 8 and sets bit 0x10 in the
//! low result byte only when the internal source level is greater than 24.
//! The exit sequence writes its result into byte 2 of this same header at
//! phase 12: action 11 yields 0x1A, and ordinary action 12 yields 0x0A.

const PLAYER_EVENT_1D_RESULT: u16 = 0x0008;
const SECRET_RESULT_BIT: u16 = 0x0010;
const SECRET_RESULT_LEVEL_THRESHOLD: u16 = 24;
const SECRET_BANISHMENT_RESULT: u16 = PLAYER_EVENT_1D_RESULT | SECRET_RESULT_BIT;

/// The eight live `dw_w_b.dword` headers and the local result view into them.
/// Source `sub_28EC0` increments the NEXT player's complete header; it does
/// not award another A4 creature kill. Keeping word[1] in this same carrier
/// preserves its alias with PlayerEvents instead of duplicating local state.
/// The owning session carries identity and hashes nonzero headers. This is
/// reset-derived state for qualified fresh sessions, not arbitrary imported
/// save/header data. `source_player_slot` is the player-array slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourcePlayerResult {
    source_player_slot: u16,
    headers: [u32; 8],
}

impl SourcePlayerResult {
    /// sub_53160 calls clean_x_D41A0_BYTEARRAY_0_0x2BDE for all eight rows.
    /// The existing fresh-session result initialization shares those zeros.
    pub(crate) const fn source_level_start(source_player_slot: u16) -> Self {
        assert!(source_player_slot < 8);
        Self {
            source_player_slot,
            headers: [0; 8],
        }
    }

    /// Record the result only when the scheduled player event 0x1D actually
    /// runs for this carrier's source player. The scheduler must call this
    /// from the already-admitted action-29 handler, after source action-slot
    /// occupancy has accepted 29. A different player identity is rejected
    /// without changing the authoritative word.
    pub(crate) fn record_player_event_0x1d(
        &mut self,
        source_player_slot: u16,
        internal_level: u16,
    ) -> bool {
        if self.source_player_slot != source_player_slot {
            return false;
        }

        let result = PLAYER_EVENT_1D_RESULT
            | if internal_level > SECRET_RESULT_LEVEL_THRESHOLD {
                SECRET_RESULT_BIT
            } else {
                0
            };
        let header = &mut self.headers[usize::from(self.source_player_slot)];
        *header = (*header & 0xffff) | (u32::from(result) << 16);
        true
    }

    pub(crate) const fn source_player_slot(&self) -> u16 {
        self.source_player_slot
    }

    /// `sub_5E8C0` phase 12 assigns byte 2, preserving the rest of the header.
    pub(crate) fn record_exit_result(&mut self, result: u8) {
        let header = &mut self.headers[usize::from(self.source_player_slot)];
        *header = (*header & !0x00ff_0000) | (u32::from(result) << 16);
    }

    pub(crate) const fn result_byte(&self) -> u8 {
        (self.headers[self.source_player_slot as usize] >> 16) as u8
    }

    /// Single-player PlayerEvents case 0x0f records the no-Castle retry in
    /// byte 2 of the current player header before invoking 5C950.
    pub(crate) fn record_single_player_no_castle_recovery(&mut self) {
        self.headers[usize::from(self.source_player_slot)] |= 0x000c_0000;
    }

    /// Return the raw source-width word, preserving ordinary result 0x0008.
    pub(crate) const fn raw_word(&self) -> u16 {
        (self.headers[self.source_player_slot as usize] >> 16) as u16
    }

    pub(crate) const fn raw_headers(&self) -> &[u32; 8] {
        &self.headers
    }

    /// Exact sub_28EC0 write, before its KillEntity/hide branch on every
    /// enabled action205 visit. The caller supplies current playerIndex,
    /// not an entity source ID or mission number. Single-player's local
    /// slot is zero. An unsupported last-slot overrun is explicit here;
    /// source memory beyond the eight-player array is not invented.
    pub(crate) fn record_creature25_kill_visit(
        &mut self,
        killer_entity: u16,
        local_player_entity: u16,
    ) -> Result<(), String> {
        if killer_entity != local_player_entity {
            return Ok(());
        }
        let next = usize::from(self.source_player_slot) + 1;
        let header = self.headers.get_mut(next).ok_or_else(|| {
            "source creature25 next-player header lies beyond the eight-player array".to_owned()
        })?;
        *header = header.wrapping_add(1);
        Ok(())
    }

    /// An ordinary action-29 result exits through the ordinary menu path,
    /// never through the secret-frame consumer.
    pub(crate) const fn is_ordinary_banishment_for(&self, source_player_slot: u16) -> bool {
        self.source_player_slot == source_player_slot && self.raw_word() == PLAYER_EVENT_1D_RESULT
    }

    /// Match only the exact secret result for the same source player. The
    /// guarded campaign consumer must additionally validate the active secret
    /// frame and runtime/session generation before returning from it.
    pub(crate) const fn is_secret_banishment_for(&self, source_player_slot: u16) -> bool {
        self.source_player_slot == source_player_slot
            && matches!(self.raw_word(), SECRET_BANISHMENT_RESULT)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PLAYER_EVENT_1D_RESULT, SECRET_BANISHMENT_RESULT, SECRET_RESULT_LEVEL_THRESHOLD,
        SourcePlayerResult,
    };

    #[test]
    fn source_event_keeps_the_full_word_and_distinguishes_ordinary_from_secret() {
        let mut ordinary = SourcePlayerResult::source_level_start(3);
        assert_eq!(ordinary.raw_word(), 0);
        assert!(ordinary.record_player_event_0x1d(3, SECRET_RESULT_LEVEL_THRESHOLD));
        assert_eq!(ordinary.raw_word(), PLAYER_EVENT_1D_RESULT);
        assert!(ordinary.is_ordinary_banishment_for(3));
        assert!(!ordinary.is_ordinary_banishment_for(4));
        assert!(!ordinary.is_secret_banishment_for(3));

        let mut secret = SourcePlayerResult::source_level_start(3);
        assert!(secret.record_player_event_0x1d(3, SECRET_RESULT_LEVEL_THRESHOLD + 1));
        assert_eq!(secret.raw_word(), SECRET_BANISHMENT_RESULT);
        assert!(!secret.is_ordinary_banishment_for(3));
        assert!(secret.is_secret_banishment_for(3));
        assert!(secret.is_secret_banishment_for(3), "query does not consume");
        assert!(!secret.is_secret_banishment_for(4));
    }

    #[test]
    fn source_event_rejects_a_different_player_without_mutating_its_result() {
        let mut result = SourcePlayerResult::source_level_start(3);
        assert!(result.record_player_event_0x1d(3, SECRET_RESULT_LEVEL_THRESHOLD + 1));
        assert!(!result.record_player_event_0x1d(4, SECRET_RESULT_LEVEL_THRESHOLD + 1));
        assert_eq!(result.source_player_slot(), 3);
        assert_eq!(result.raw_word(), SECRET_BANISHMENT_RESULT);
        assert!(result.is_secret_banishment_for(3));
    }

    #[test]
    fn new_level_reset_is_zero_and_clones_preserve_the_source_word() {
        let mut source_session = SourcePlayerResult::source_level_start(1);
        assert!(source_session.record_player_event_0x1d(1, SECRET_RESULT_LEVEL_THRESHOLD + 1));
        assert_eq!(source_session, source_session.clone());

        let next_source_session = SourcePlayerResult::source_level_start(1);
        assert_eq!(next_source_session.raw_word(), 0);
        assert_eq!(source_session.raw_word(), SECRET_BANISHMENT_RESULT);
    }

    #[test]
    fn creature25_visits_change_next_header_without_duplicating_the_local_result() {
        let mut state = SourcePlayerResult::source_level_start(0);
        assert!(state.record_player_event_0x1d(0, 25));
        state.record_creature25_kill_visit(94, 95).unwrap();
        assert_eq!(state.headers[1], 0);
        for _ in 0..257 {
            state.record_creature25_kill_visit(95, 95).unwrap();
        }
        assert_eq!(state.headers[1], 0x101);
        assert_eq!(state.headers[2..], [0; 6]);
        assert_eq!(state.raw_word(), SECRET_BANISHMENT_RESULT);
        assert!(state.is_secret_banishment_for(0));
        assert_eq!(state, state.clone());
        // The source compares words literally, without an extra nonzero gate.
        state.record_creature25_kill_visit(0, 0).unwrap();
        assert_eq!(state.headers[1], 0x102);
    }

    #[test]
    fn creature25_header_wrap_and_result_word_alias_keep_source_widths() {
        let mut state = SourcePlayerResult::source_level_start(2);
        state.headers[2] = 0xabcd_9876;
        state.headers[3] = u32::MAX;
        assert!(state.record_player_event_0x1d(2, 24));
        assert_eq!(state.headers[2], 0x0008_9876);
        state.record_creature25_kill_visit(90, 90).unwrap();
        assert_eq!(state.headers[3], 0);
        assert_eq!(state.raw_word(), PLAYER_EVENT_1D_RESULT);
        let mut last = SourcePlayerResult::source_level_start(7);
        last.record_creature25_kill_visit(90, 91).unwrap();
        assert!(last.record_creature25_kill_visit(91, 91).is_err());
        assert_eq!(last.raw_headers(), &[0; 8]);
    }

    #[test]
    fn exit_result_replaces_only_the_source_result_byte() {
        let mut state = SourcePlayerResult::source_level_start(2);
        state.headers[2] = 0xabcd_9876;
        state.record_exit_result(0x0a);
        assert_eq!(state.headers[2], 0xab0a_9876);
        assert_eq!(state.result_byte(), 0x0a);
        state.record_exit_result(0x1a);
        assert_eq!(state.headers[2], 0xab1a_9876);
    }

    #[test]
    fn secret_package_menu_commands_produce_the_secret_result_for_the_live_player() {
        use crate::{
            campaign::backend::{Mc2CampaignAdapter, Mc2CampaignInputs, load_mc2_campaign},
            game_definition::default_catalog,
            game_session::GameSession,
        };
        use sim_session::{AbandonAction, GameCommand};
        use std::{path::Path, sync::Arc};

        // This opens a real secret package directly to exercise its caller.
        // It does not claim an earned portal entry or campaign return.
        let package = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc-paj2-source-secret-31-campaign-package");
        let definition =
            Arc::new(load_mc2_campaign(&package, &default_catalog().unwrap()).unwrap());
        let mut session = GameSession::open(
            &Mc2CampaignAdapter,
            definition,
            7131,
            Mc2CampaignInputs::new(&package),
        )
        .unwrap();
        let stale = session.identity().guard();
        let command = |action| GameCommand {
            abandon_action: Some(action),
            ..GameCommand::default()
        };
        let wrong = session
            .step(stale, command(AbandonAction::ConfirmAbandon))
            .unwrap();
        assert!(!wrong.value.source_menu_admission.unwrap().admitted);
        assert!(
            !session
                .backend()
                .compatibility_campaign()
                .simulation()
                .secret_banishment_result()
        );
        let before = session.snapshot();
        assert!(
            session
                .step(stale, command(AbandonAction::OpenAbandonConfirmation))
                .is_err()
        );
        assert_eq!(session.snapshot(), before);
        let opened = session
            .step(
                session.identity().guard(),
                command(AbandonAction::OpenAbandonConfirmation),
            )
            .unwrap();
        assert!(opened.value.source_menu_admission.unwrap().admitted);
        let confirmed = session
            .step(
                session.identity().guard(),
                command(AbandonAction::ConfirmAbandon),
            )
            .unwrap();
        assert!(confirmed.value.source_menu_admission.unwrap().admitted);
        let simulation = session.backend().compatibility_campaign().simulation();
        assert!(simulation.secret_banishment_result());
        assert!(!simulation.ordinary_banishment_result());
        assert!(!confirmed.value.return_to_title_requested);
    }
}
