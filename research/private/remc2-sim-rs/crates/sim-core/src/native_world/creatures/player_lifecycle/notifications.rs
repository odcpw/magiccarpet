//! Per-participant death/banishment notices and their presentation-frame clock.
//!
//! Gameplay publishes a notice before clearing participation. Presentation may
//! then continue to show it after the participant's actor has disappeared.

use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PlayerLifeNoticeKind {
    Died,
    Banished,
}

impl PlayerLifeNoticeKind {
    pub(crate) const fn semantic_name(self) -> &'static str {
        match self {
            Self::Died => "participant.died",
            Self::Banished => "participant.banished",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PlayerLifeNotice {
    pub(crate) kind: Option<PlayerLifeNoticeKind>,
    /// Participant identity at publication; never used to dereference an actor.
    pub(crate) source_owner: u16,
    pub(crate) state: u16,
    pub(crate) remaining_frames: i16,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PlayerLifecycleNotifications {
    entries: [PlayerLifeNotice; 8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NoticePlayerOutOfRange(pub(crate) u8);

impl std::fmt::Display for NoticePlayerOutOfRange {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "notice participant {} is outside 0..8", self.0)
    }
}

impl std::error::Error for NoticePlayerOutOfRange {}

impl PlayerLifecycleNotifications {
    pub(crate) fn publish(
        &mut self,
        player_color: u8,
        source_owner: u16,
        kind: PlayerLifeNoticeKind,
        frames: i16,
    ) -> Result<(), NoticePlayerOutOfRange> {
        let entry = self
            .entries
            .get_mut(usize::from(player_color))
            .ok_or(NoticePlayerOutOfRange(player_color))?;
        *entry = PlayerLifeNotice {
            kind: Some(kind),
            source_owner,
            state: 1,
            remaining_frames: frames,
        };
        Ok(())
    }

    /// One admitted DrawTextPauseEndOfLevel frame, not an entity update.
    /// F10/settings and selected dialogs suppress this work; pause alone does
    /// not. The caller supplies the same admission used by the actual draw.
    pub(crate) fn presentation_frame(&mut self, notifications_visible: bool) {
        if !notifications_visible {
            return;
        }
        for entry in &mut self.entries {
            if entry.state != 1 {
                continue;
            }
            entry.remaining_frames = entry.remaining_frames.wrapping_sub(1);
            // Source decrements the signed word first, then promotes for +1.
            // A timer reaching zero is still drawn on this frame.
            if i32::from(entry.remaining_frames) + 1 <= 0 {
                entry.state = 0;
            }
        }
    }

    pub(crate) fn entries(&self) -> &[PlayerLifeNotice; 8] {
        &self.entries
    }

    pub(crate) fn append_hash(&self, hash: &mut Sha256) {
        // An untouched presentation domain keeps the existing empty reset
        // encoding. Any publication, including retained expired data, versions
        // and hashes the complete per-participant payload below.
        if self
            .entries
            .iter()
            .all(|entry| *entry == PlayerLifeNotice::default())
        {
            hash.update([0]);
            return;
        }
        hash.update([1]);
        hash.update(b"per-participant-life-notices-v1");
        for entry in &self.entries {
            hash.update([match entry.kind {
                None => 0,
                Some(PlayerLifeNoticeKind::Died) => 1,
                Some(PlayerLifeNoticeKind::Banished) => 2,
            }]);
            hash.update(entry.source_owner.to_le_bytes());
            hash.update(entry.state.to_le_bytes());
            hash.update(entry.remaining_frames.to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PlayerLifeNoticeKind, PlayerLifecycleNotifications};

    #[test]
    fn source_zero_timer_still_draws_and_banishment_keeps_its_own_participant() {
        let mut notices = PlayerLifecycleNotifications::default();
        notices
            .publish(0, 111, PlayerLifeNoticeKind::Died, 1)
            .unwrap();
        notices
            .publish(7, 425, PlayerLifeNoticeKind::Banished, 200)
            .unwrap();
        let before = notices.clone();
        notices.presentation_frame(false);
        assert_eq!(notices, before, "a hidden source notice frame does not age");
        notices.presentation_frame(true);
        assert_eq!(
            (
                notices.entries()[0].state,
                notices.entries()[0].remaining_frames
            ),
            (1, 0),
            "the last source frame remains visible"
        );
        assert_eq!(
            (
                notices.entries()[7].source_owner,
                notices.entries()[7].kind,
                notices.entries()[7].remaining_frames,
            ),
            (425, Some(PlayerLifeNoticeKind::Banished), 199)
        );
        notices.presentation_frame(true);
        assert_eq!(
            (
                notices.entries()[0].state,
                notices.entries()[0].remaining_frames
            ),
            (0, -1)
        );
        assert_eq!(notices.entries()[0].kind, Some(PlayerLifeNoticeKind::Died));
        assert_eq!(notices.entries()[7].state, 1);
        let before_rejection = notices.clone();
        assert!(
            notices
                .publish(8, 0, PlayerLifeNoticeKind::Died, 1)
                .is_err()
        );
        assert_eq!(notices, before_rejection);

        // A configured word-width discriminator: narrow decrement precedes
        // the promoted comparison, even at the signed-word boundary.
        notices
            .publish(0, 111, PlayerLifeNoticeKind::Died, i16::MIN)
            .unwrap();
        notices.presentation_frame(true);
        assert_eq!(
            (
                notices.entries()[0].state,
                notices.entries()[0].remaining_frames
            ),
            (1, i16::MAX)
        );
    }
}
