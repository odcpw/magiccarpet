//! Source campaign-return cutscene history and credits lifetime.
use serde::Serialize;

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
pub struct CreditsSnapshot {
    pub lines: Vec<String>,
    pub cursor: usize,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
pub struct CampaignReturnSnapshot {
    pub phase: &'static str,
    pub blocking: bool,
    pub cutscene_id: Option<u8>,
    pub credits: Option<CreditsSnapshot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase { Cutscene, CreditsEntryDrain, CreditsFadeIn, Credits, CreditsFadeOut, ExitDrain }

#[derive(Clone, Debug, Default)]
pub(super) struct CampaignReturn {
    played: u8,
    phase: Option<Phase>,
    cutscene: Option<u8>,
    credits_pending: bool,
    cursor: usize,
    displayed_cursor: usize,
    elapsed_us: u64,
    fade_us: u64,
    started: bool,
}

impl CampaignReturn {
    pub(super) fn played(&self) -> u8 { self.played }
    pub(super) fn restore_played(&mut self, played: u8) -> Result<(), String> {
        if played & !0x3f != 0 { return Err("invalid cutscene history".into()); }
        self.played = played;
        Ok(())
    }
    pub(super) fn reset(&mut self) { *self = Self::default(); }
    pub(super) fn active(&self) -> bool { self.phase.is_some() }
    pub(super) fn is_cutscene(&self) -> bool { self.phase == Some(Phase::Cutscene) }
    pub(super) fn credits_pending(&self) -> bool { self.credits_pending }
    pub(super) fn exit_drain(&self) -> bool { self.phase == Some(Phase::ExitDrain) }
    pub(super) fn needs_start(&self) -> bool { self.is_cutscene() && !self.started }
    pub(super) fn mark_started(&mut self) { self.started = true; }
    pub(super) fn cutscene(&self) -> Option<u8> { self.cutscene }
    pub(super) fn queue(&mut self, cutscene: Option<u8>, credits: bool) {
        let cutscene = cutscene.filter(|id| (1..=6).contains(id))
            .filter(|id| self.played & (1 << (id - 1)) == 0);
        if let Some(id) = cutscene { self.played |= 1 << (id - 1); }
        self.cutscene = cutscene;
        self.credits_pending = credits;
        self.cursor = 0;
        self.displayed_cursor = 0;
        self.elapsed_us = 0;
        self.fade_us = 0;
        self.started = false;
        self.phase = if cutscene.is_some() { Some(Phase::Cutscene) }
            else if credits { Some(Phase::CreditsEntryDrain) } else { None };
    }
    pub(super) fn cutscene_done(&mut self) {
        self.phase = Some(if self.credits_pending { Phase::CreditsEntryDrain } else { Phase::ExitDrain });
        self.elapsed_us = 0;
    }
    pub(super) fn unavailable(&mut self) {
        self.credits_pending = false;
        self.phase = Some(Phase::ExitDrain);
    }
    fn draw_credits(&mut self, elapsed_us: u32, records: &[String]) {
        // ShowEndCredits draws index2 first, then advances index2 for the
        // next frame; retain the page actually drawn this frame.
        self.displayed_cursor = self.cursor;
        self.elapsed_us = self.elapsed_us.saturating_add(u64::from(elapsed_us));
        let end = records[self.cursor..].iter().position(|line| line == "!")
            .map(|n| self.cursor + n);
        if let Some(end) = end {
            if self.elapsed_us >= 3_000_000 { self.cursor = end + 1; self.elapsed_us = 0; }
        } else if self.elapsed_us >= 6_000_000 {
            self.cursor = 0; self.elapsed_us = 0;
        }
    }
    pub(super) fn frame(&mut self, elapsed_us: u32, input: bool, records: &[String]) {
        match self.phase {
            Some(Phase::CreditsEntryDrain) => {
                self.elapsed_us = self.elapsed_us.saturating_add(u64::from(elapsed_us));
                if !input {
                    self.draw_credits(0, records);
                    self.phase = Some(Phase::CreditsFadeIn);
                    self.fade_us = 0;
                }
            }
            Some(Phase::CreditsFadeIn) => {
                self.fade_us = self.fade_us.saturating_add(u64::from(elapsed_us));
                self.elapsed_us = self.elapsed_us.saturating_add(u64::from(elapsed_us));
                if self.fade_us >= 320_000 { self.phase = Some(Phase::Credits); }
            }
            Some(Phase::Credits) if input => {
                self.draw_credits(elapsed_us, records);
                self.phase = Some(Phase::CreditsFadeOut);
                self.fade_us = 0;
            }
            Some(Phase::CreditsFadeOut) => {
                self.fade_us = self.fade_us.saturating_add(u64::from(elapsed_us));
                if self.fade_us >= 160_000 { self.phase = Some(Phase::ExitDrain); }
            }
            Some(Phase::Credits) => {
                self.draw_credits(elapsed_us, records);
            }
            Some(Phase::ExitDrain) if !input => self.phase = None,
            _ => {}
        }
    }
    pub(super) fn snapshot(&self, records: &[String]) -> Option<CampaignReturnSnapshot> {
        let phase = self.phase?;
        let showing_credits = matches!(phase, Phase::CreditsFadeIn | Phase::Credits | Phase::CreditsFadeOut);
        let credits = showing_credits.then(|| CreditsSnapshot {
            lines: records[self.displayed_cursor..].iter().take_while(|line| *line != "!").cloned().collect(),
            cursor: self.displayed_cursor,
        });
        Some(CampaignReturnSnapshot {
            phase: match phase { Phase::Cutscene => "cutscene", Phase::CreditsFadeIn | Phase::Credits | Phase::CreditsFadeOut => "credits", _ => "input_release" },
            blocking: true,
            cutscene_id: self.cutscene,
            credits,
        })
    }
    pub(super) fn fade_levels(&self) -> Option<(u32, u32)> {
        match self.phase {
            Some(Phase::CreditsFadeIn) => Some(((self.fade_us / 10_000).min(31) as u32, 0)),
            // The source's non-single-step fade installs 0..31 and does not
            // install level 32 before starting the later fade to black.
            Some(Phase::Credits) => Some((31, 0)),
            Some(Phase::CreditsFadeOut) => Some((31, (self.fade_us / 10_000).min(15) as u32)),
            _ => None,
        }
    }
}

pub(crate) fn milestone(index: u16) -> Option<u8> {
    [4, 8, 12, 16, 23, 24].iter().position(|item| *item == index).map(|i| i as u8 + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn played_cut_is_marked_before_playback_and_credits_wait_for_release() {
        let records = vec!["first".into(), "!".into(), "last".into()];
        let mut owner = CampaignReturn::default();
        owner.queue(milestone(24), true);
        assert_eq!(owner.played(), 32);
        assert_eq!(owner.snapshot(&records).unwrap().phase, "cutscene");
        owner.cutscene_done();
        owner.frame(2_000_000, true, &records);
        owner.frame(1_000_000, false, &records);
        assert_eq!(owner.snapshot(&records).unwrap().credits.unwrap().lines, ["first"]);
        assert_eq!(owner.fade_levels(), Some((0, 0)));
        owner.frame(320_000, false, &records);
        assert_eq!(owner.fade_levels(), Some((31, 0)));
        assert_eq!(owner.snapshot(&records).unwrap().credits.unwrap().lines, ["first"]);
        owner.frame(0, false, &records);
        assert_eq!(owner.snapshot(&records).unwrap().credits.unwrap().lines, ["last"]);
        owner.frame(5_679_999, false, &records);
        assert_eq!(owner.snapshot(&records).unwrap().credits.unwrap().lines, ["last"]);
        owner.frame(1, false, &records);
        assert_eq!(owner.snapshot(&records).unwrap().credits.unwrap().lines, ["last"]);
        owner.frame(0, false, &records);
        assert_eq!(owner.snapshot(&records).unwrap().credits.unwrap().lines, ["first"]);
        owner.frame(0, true, &records);
        assert_eq!(owner.fade_levels(), Some((31, 0)));
        assert!(owner.active());
        owner.frame(160_000, true, &records);
        assert!(owner.active());
        owner.frame(0, true, &records);
        assert!(owner.active());
        owner.frame(0, false, &records);
        assert!(!owner.active());
        owner.queue(milestone(24), false);
        assert!(!owner.active(), "played CUT must not restart");
    }

    #[test]
    fn history_restore_rejects_invalid_bits_and_suppresses_replay() {
        let mut owner = CampaignReturn::default();
        assert!(owner.restore_played(0x40).is_err());
        assert_eq!(owner.played(), 0);
        owner.restore_played(1).unwrap();
        owner.queue(milestone(4), false);
        assert!(!owner.active());
        owner.queue(milestone(8), false);
        assert_eq!(owner.played(), 3);
        assert!(owner.active());
    }
}
