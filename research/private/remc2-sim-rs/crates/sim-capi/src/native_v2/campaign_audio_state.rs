//! Device-side voice ownership for the runtime's retained source audio events.
//! Gameplay, positions and sound preparation remain in the campaign journal.

use super::*;
use sim_core::campaign::{CampaignSession, journal::EffectCursor};

#[derive(Default)]
pub(super) struct CampaignRuntimeAudio {
    pub(super) generation: u64,
    cursor: u64,
    pub(super) commands: BTreeMap<u64, McAudioCommandV2>,
    pub(super) voices: [Option<AudioVoice>; AUDIO_VOICE_COUNT],
}

impl CampaignRuntimeAudio {
    pub(super) fn reset(&mut self, campaign: &CampaignSession, generation: u64) {
        *self = Self {
            generation,
            cursor: campaign.effect_cursor().next_sequence,
            ..Self::default()
        };
    }

    pub(super) fn sync(
        &mut self,
        campaign: &CampaignSession,
        generation: u64,
        paused: bool,
    ) -> AbiResult<Vec<(u8, u8)>> {
        if self.generation != generation {
            // Pause clears voices, while resume explicitly rebinds. A
            // remaining generation change is a real session reset; include
            // the first newly produced frame instead of skipping its events.
            *self = Self {
                generation,
                ..Self::default()
            };
        }
        let page = campaign
            .effects_after(
                EffectCursor {
                    generation: campaign.generation(),
                    next_sequence: self.cursor,
                },
                16_384,
            )
            .map_err(campaign_effect_page_error)?;
        let state_hash = campaign.state_hash();
        let mut music_updates = Vec::new();
        for record in &page.records {
            if let sim_core::campaign::journal::CampaignEffect::Flight(FlightEvent::UpdateMusic {
                mode,
                timer_multiplier,
            }) = record.effect
            {
                music_updates.push((mode, timer_multiplier));
            }
            let Some(prepared) = record.audio else {
                continue;
            };
            let (entity_index, player_index, sound_id, ending) = match record.effect {
                sim_core::campaign::journal::CampaignEffect::Flight(
                    FlightEvent::PrepareSound {
                        entity_index,
                        player_index,
                        sound_id,
                        ..
                    },
                ) => (entity_index, player_index, sound_id, false),
                sim_core::campaign::journal::CampaignEffect::Flight(FlightEvent::EndLoop {
                    entity_index,
                    player_index,
                    sound_id,
                    ..
                }) => (entity_index, player_index, sound_id, true),
                _ => continue,
            };
            let sequence = record.sequence.wrapping_add(1);
            let mut command = audio_command_record(
                generation,
                sequence,
                state_hash,
                u32::from(sound_id),
                sound_id,
                entity_index,
                player_index,
                prepared.command,
                prepared.play_type,
            );
            if matches!(sound_id, 1 | 2 | 5 | 31) {
                command.loop_count = -1;
            }
            if ending {
                let matching = self.voices.iter().position(|voice| {
                    voice.is_some_and(|voice| {
                        voice.sound_id == sound_id && voice.sound_flags == command.sound_flags
                    })
                });
                if let Some(index) = matching {
                    command.action = MC_AUDIO_ACTION_END_LOOP;
                    command.voice_index = index as u16;
                } else {
                    command.action = MC_AUDIO_ACTION_SUPPRESS_NO_EXISTING;
                }
            }
            self.commands.insert(sequence, command);
        }
        self.cursor = page.next_cursor.next_sequence;
        if !paused {
            super::audio_voice_pool::drain_pending_audio_commands(
                &mut self.commands,
                &mut self.voices,
            );
        }
        while self.commands.len() > 16_384 {
            self.commands.pop_first();
        }
        Ok(music_updates)
    }

    pub(super) fn validate_generation(&self, generation: u64) -> AbiResult<()> {
        if generation != self.generation {
            return Err(AbiError::new(
                MC_STATUS_GENERATION_MISMATCH,
                "campaign audio generation mismatch",
            ));
        }
        Ok(())
    }
}
