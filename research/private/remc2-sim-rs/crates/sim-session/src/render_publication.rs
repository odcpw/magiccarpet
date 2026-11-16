//! Transport-neutral binary render publication, version 4.
//!
//! Integers are little endian with no padding. Strings are UTF-8 prefixed by a
//! u32 byte length; vectors use a u32 count; options and booleans use one byte.
//! The fixed prefix is `MCPO`, version u32, packet byte length u32, runtime ID
//! u64, runtime generation u64, and mode u8 (0 playing, 1 paused). Remaining
//! fields are written in the declaration order below and in the session types.
use crate::{
    AbandonAction, CameraPose, ObjectiveProgress, ParticipantNotification, PlayerMotion,
    PlayerPose, PlayerVitals, PresentationEntity, SessionIdentity, SourceMenuAdmission,
    SourceScoreboard, SpellAvailability, SpellHand, SpellMarkHand, SpellObservation,
};

/// Borrowed live presentation data. The producer owns the underlying state and
/// must acquire this entire view at one authoritative session boundary.
pub struct RenderPublication<'a> {
    pub runtime_id: u64,
    pub runtime_generation: u64,
    pub paused: bool,
    pub identity: &'a SessionIdentity,
    pub terrain_revision: u64,
    pub tick: u64,
    pub effect_head: u64,
    pub player_pose: &'a PlayerPose,
    pub player_motion: &'a PlayerMotion,
    pub player_vitals: &'a PlayerVitals,
    pub source_menu_admission: Option<&'a SourceMenuAdmission>,
    pub return_to_title_requested: bool,
    pub source_scoreboard: &'a SourceScoreboard,
    pub participant_notifications: &'a [ParticipantNotification],
    pub player_wall_contact_counter: i8,
    pub player_immobilized_ticks: u16,
    pub source_camera_pose: &'a CameraPose,
    pub source_camera_fov: i16,
    pub spell_menu_has_any_spell: bool,
    pub spell_menu_wizard_colors: [u8; 2],
    pub spell_menu_panels_opaque: bool,
    pub player_spells: &'a [SpellObservation],
    pub objective_progress: &'a ObjectiveProgress,
    pub objective_textbox_ticks: u8,
    /// Zero on the wire denotes no special objective-text record.
    pub objective_secret_announcement_record: Option<u16>,
    pub exit_sequence_active: bool,
    pub presentation_entities: &'a [PresentationEntity],
    /// Source global [left, right] visibility counters, each in 0..=8.
    pub player_hand_selection_flash_ticks: [u8; 2],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EncodeError {
    /// A length, count, or complete packet cannot be represented by u32.
    LengthOverflow,
    /// No bytes were written; the argument is the required packet length.
    BufferTooSmall(u32),
    /// A producer supplied a counter outside the source eight-update window.
    InvalidHandSelectionFlashTicks([u8; 2]),
}

struct Writer<'a> {
    bytes: Option<&'a mut [u8]>,
    offset: usize,
}
impl Writer<'_> {
    fn raw(&mut self, value: &[u8]) -> Result<(), EncodeError> {
        let end = self
            .offset
            .checked_add(value.len())
            .ok_or(EncodeError::LengthOverflow)?;
        if end > u32::MAX as usize {
            return Err(EncodeError::LengthOverflow);
        }
        if let Some(bytes) = &mut self.bytes {
            bytes[self.offset..end].copy_from_slice(value);
        }
        self.offset = end;
        Ok(())
    }
    fn u8(&mut self, v: u8) -> Result<(), EncodeError> {
        self.raw(&[v])
    }
    fn bool(&mut self, v: bool) -> Result<(), EncodeError> {
        self.u8(u8::from(v))
    }
    fn i8(&mut self, v: i8) -> Result<(), EncodeError> {
        self.u8(v as u8)
    }
    fn u16(&mut self, v: u16) -> Result<(), EncodeError> {
        self.raw(&v.to_le_bytes())
    }
    fn i16(&mut self, v: i16) -> Result<(), EncodeError> {
        self.raw(&v.to_le_bytes())
    }
    fn u32(&mut self, v: u32) -> Result<(), EncodeError> {
        self.raw(&v.to_le_bytes())
    }
    fn i32(&mut self, v: i32) -> Result<(), EncodeError> {
        self.raw(&v.to_le_bytes())
    }
    fn u64(&mut self, v: u64) -> Result<(), EncodeError> {
        self.raw(&v.to_le_bytes())
    }
    fn count(&mut self, n: usize) -> Result<(), EncodeError> {
        self.u32(u32::try_from(n).map_err(|_| EncodeError::LengthOverflow)?)
    }
    fn string(&mut self, v: &str) -> Result<(), EncodeError> {
        self.count(v.len())?;
        self.raw(v.as_bytes())
    }
    fn optional<T>(
        &mut self,
        value: Option<T>,
        f: impl FnOnce(&mut Self, T) -> Result<(), EncodeError>,
    ) -> Result<(), EncodeError> {
        self.bool(value.is_some())?;
        if let Some(value) = value {
            f(self, value)?;
        }
        Ok(())
    }
}

impl RenderPublication<'_> {
    /// Returns the exact byte size without touching a caller buffer.
    pub fn encoded_len(&self) -> Result<u32, EncodeError> {
        let mut writer = Writer {
            bytes: None,
            offset: 0,
        };
        self.write(&mut writer, 0)?;
        u32::try_from(writer.offset).map_err(|_| EncodeError::LengthOverflow)
    }

    /// Write exactly one packet, or leave the entire buffer unchanged.
    pub fn encode_into(&self, buffer: &mut [u8]) -> Result<u32, EncodeError> {
        let length = self.encoded_len()?;
        if buffer.len() < length as usize {
            return Err(EncodeError::BufferTooSmall(length));
        }
        let mut writer = Writer {
            bytes: Some(buffer),
            offset: 0,
        };
        self.write(&mut writer, length)?;
        Ok(length)
    }

    fn write(&self, w: &mut Writer<'_>, length: u32) -> Result<(), EncodeError> {
        if self.player_hand_selection_flash_ticks.iter().any(|&value| value > 8) {
            return Err(EncodeError::InvalidHandSelectionFlashTicks(self.player_hand_selection_flash_ticks));
        }
        w.raw(b"MCPO")?;
        w.u32(4)?;
        w.u32(length)?;
        w.u64(self.runtime_id)?;
        w.u64(self.runtime_generation)?;
        w.bool(self.paused)?;
        let id = self.identity;
        w.string(id.engine_contract)?;
        w.string(&id.package_graph_hash)?;
        w.string(&id.definition_hash)?;
        w.u64(id.definition_revision)?;
        w.u64(id.session_id)?;
        w.u64(id.generation)?;
        w.u64(id.tick)?;
        w.u64(self.terrain_revision)?;
        w.u64(self.tick)?;
        w.u64(self.effect_head)?;
        let p = self.player_pose;
        w.u16(p.x)?;
        w.u16(p.y)?;
        w.i16(p.z)?;
        w.i16(p.yaw)?;
        w.i16(p.display_pitch)?;
        w.i16(p.effective_pitch)?;
        w.i16(p.roll)?;
        let m = self.player_motion;
        w.i16(m.target_speed)?;
        w.i16(m.actual_speed)?;
        w.i16(m.strafe_speed)?;
        let v = self.player_vitals;
        w.i32(v.life)?;
        w.i32(v.max_life)?;
        w.i32(v.mana)?;
        w.i32(v.max_mana)?;
        w.optional(self.source_menu_admission, |w, a| {
            w.u8(match a.action {
                AbandonAction::ToggleGameplayMap => 0,
                AbandonAction::OpenAbandonConfirmation => 1,
                AbandonAction::ConfirmAbandon => 2,
                AbandonAction::CancelAbandonConfirmation => 3,
                AbandonAction::OpenSpellMenu => 4,
                AbandonAction::CloseSpellMenu => 5,
                AbandonAction::ShowPlayerInformation => 6,
                AbandonAction::HidePlayerInformation => 7,
                AbandonAction::CloseGameplayMenus => 8,
            })?;
            w.bool(a.admitted)?;
            w.bool(a.map_open)?;
            w.bool(a.spells_open)?;
            w.bool(a.player_information_open)?;
            w.u64(a.tick)
        })?;
        w.bool(self.return_to_title_requested)?;
        w.count(self.source_scoreboard.players.len())?;
        for s in &self.source_scoreboard.players {
            w.u8(s.source_slot)?;
            w.string(&s.display_name)?;
            w.u8(s.outline_palette_index)?;
            w.u8(s.text_palette_index)?;
            w.i32(s.max_mana)?;
            for k in s.kills8 {
                w.i16(k)?;
            }
        }
        w.optional(
            self.source_scoreboard.projection_error.as_deref(),
            |w, s| w.string(s),
        )?;
        w.count(self.participant_notifications.len())?;
        for n in self.participant_notifications {
            w.u64(n.participant.0)?;
            w.bool(n.is_local)?;
            w.string(&n.display_name)?;
            w.string(n.kind.as_str())?;
            w.i16(n.remaining_frames)?;
        }
        w.i8(self.player_wall_contact_counter)?;
        w.u16(self.player_immobilized_ticks)?;
        let c = self.source_camera_pose;
        w.u16(c.x)?;
        w.u16(c.y)?;
        w.i16(c.z)?;
        w.i16(c.yaw)?;
        w.i16(c.pitch)?;
        w.i16(c.roll)?;
        w.i16(self.source_camera_fov)?;
        w.bool(self.spell_menu_has_any_spell)?;
        for color in self.spell_menu_wizard_colors {
            w.u8(color)?;
        }
        w.bool(self.spell_menu_panels_opaque)?;
        w.count(self.player_spells.len())?;
        for s in self.player_spells {
            w.string(s.id.as_str())?;
            w.string(&s.display_name)?;
            w.bool(s.acquired)?;
            w.optional(s.holder_actor, |w, actor| w.u64(actor.0))?;
            w.bool(s.selected)?;
            w.i8(s.tier)?;
            w.u8(s.unlocked_tier)?;
            w.u8(s.menu_slot)?;
            w.optional(s.menu_artwork.as_ref(), |w, a| {
                w.u16(a.grid_icon)?;
                for icon in a.tier_icons {
                    w.u16(icon)?;
                }
                Ok(())
            })?;
            w.optional(s.menu_help.as_ref(), |w, help| {
                w.bool(help.is_standard)?;
                w.i8(help.selected_tier)?;
                for record in help.title_records {
                    w.u16(record)?;
                }
                for blocked in help.castle_blocked {
                    w.bool(blocked)?;
                }
                w.bool(help.mana_unavailable)?;
                w.bool(help.definition_enabled)?;
                w.bool(help.blink_hidden)?;
                w.bool(help.discovered)?;
                for cost in help.tier_mana_costs {
                    w.i32(cost)?;
                }
                w.i32(help.experience)?;
                for threshold in help.experience_thresholds {
                    w.i32(threshold)?;
                }
                Ok(())
            })?;
            w.i32(s.mana_cost)?;
            w.u8(match s.hand {
                SpellHand::None => 0,
                SpellHand::Left => 1,
                SpellHand::Right => 2,
                SpellHand::Both => 3,
            })?;
            w.u8(match s.marked_hand {
                SpellMarkHand::None => 0,
                SpellMarkHand::Left => 1,
                SpellMarkHand::Right => 2,
            })?;
            w.u8(match s.availability {
                SpellAvailability::Ready => 0,
                SpellAvailability::NotAcquired => 1,
                SpellAvailability::InvalidHolder => 2,
                SpellAvailability::InvalidCaster => 3,
                SpellAvailability::Active => 4,
                SpellAvailability::InsufficientMana => 5,
            })?;
        }
        let o = self.objective_progress;
        w.optional(o.current_objective.as_ref(), |w, id| w.string(id.as_str()))?;
        w.u32(o.completed_count)?;
        w.u32(o.total_count)?;
        w.bool(o.all_objectives_satisfied)?;
        w.u8(self.objective_textbox_ticks)?;
        w.u16(self.objective_secret_announcement_record.unwrap_or(0))?;
        w.bool(self.exit_sequence_active)?;
        w.count(self.presentation_entities.len())?;
        for e in self.presentation_entities {
            w.u64(e.actor.0)?;
            w.string(e.kind.as_str())?;
            w.u16(e.pose.x)?;
            w.u16(e.pose.y)?;
            w.i16(e.pose.z)?;
            w.i16(e.pose.yaw)?;
            w.i16(e.pose.pitch)?;
            w.u16(e.logical_height)?;
            w.string(&e.asset.id)?;
            w.string(&e.asset.content_address)?;
        }
        w.raw(&self.player_hand_selection_flash_ticks)?;
        Ok(())
    }
}
