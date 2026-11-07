//! Device-side voice ownership for the runtime's retained source audio events.
//! Gameplay, positions and sound preparation remain in the campaign journal.

use super::campaign_sound_bank::CampaignSoundBank;
use super::*;
use sim_core::campaign::{CampaignSession, journal::EffectCursor};

fn suppress_null_sample(
    bank: Option<&CampaignSoundBank>,
    command: &mut McAudioCommandV2,
) -> AbiResult<bool> {
    if let Some(bank) = bank
        && bank.is_null(command.sound_id)?
    {
        command.action = MC_AUDIO_ACTION_SUPPRESS_NULL_SAMPLE;
        command.voice_index = MC_AUDIO_VOICE_NONE;
        return Ok(true);
    }
    Ok(false)
}

#[derive(Default)]
pub(super) struct CampaignRuntimeAudio {
    pub(super) generation: u64,
    cursor: u64,
    pub(super) commands: BTreeMap<u64, McAudioCommandV2>,
    pub(super) voices: [Option<AudioVoice>; AUDIO_VOICE_COUNT],
    sound_bank: Option<CampaignSoundBank>,
}

impl CampaignRuntimeAudio {
    pub(super) fn bind_sound_bank(&mut self, bank: CampaignSoundBank) -> AbiResult<()> {
        if self
            .sound_bank
            .as_ref()
            .is_some_and(|current| current != &bank)
            && (self.voices.iter().any(Option::is_some) || !self.commands.is_empty())
        {
            return Err(AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "cannot replace a sound bank while its generation owns voices or commands",
            ));
        }
        self.sound_bank = Some(bank);
        Ok(())
    }

    pub(super) fn clear_sound_bank(&mut self) -> AbiResult<()> {
        if self.voices.iter().any(Option::is_some) || !self.commands.is_empty() {
            return Err(AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "cannot clear a sound bank while its generation owns voices or commands",
            ));
        }
        self.sound_bank = None;
        Ok(())
    }

    pub(super) fn reset(&mut self, campaign: &CampaignSession, generation: u64) {
        let sound_bank = self.sound_bank.take();
        *self = Self {
            generation,
            cursor: campaign.effect_cursor().next_sequence,
            sound_bank,
            ..Self::default()
        };
    }

    pub(super) fn sync(
        &mut self,
        campaign: &CampaignSession,
        generation: u64,
        paused: bool,
    ) -> AbiResult<Vec<(u8, u8)>> {
        if self
            .sound_bank
            .as_ref()
            .is_some_and(|bank| bank.selection() != campaign.definition().identity)
        {
            return Err(AbiError::new(
                MC_STATUS_VALIDATION_FAILED,
                "selected sound bank belongs to a different campaign",
            ));
        }
        if self.generation != generation {
            // Pause clears voices, while resume explicitly rebinds. A
            // remaining generation change is a real session reset; include
            // the first newly produced frame instead of skipping its events.
            let sound_bank = self.sound_bank.take();
            *self = Self {
                generation,
                sound_bank,
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
        // Music-only and empty pages need no gameplay fingerprint. The
        // campaign is immutably borrowed for this entire synchronization.
        let mut state_hash = None;
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
                *state_hash.get_or_insert_with(|| campaign.state_hash()),
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
            if !ending {
                suppress_null_sample(self.sound_bank.as_ref(), &mut command)?;
            } else {
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

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::campaign::spec::CampaignSelection;
    use std::path::PathBuf;

    fn actual_bank(cache: &str, level: u16) -> CampaignSoundBank {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated")
            .join(cache);
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("manifest.json")).unwrap()).unwrap();
        let profile = &manifest["profile"];
        let owner = profile["owner_export_package_sha256"].as_str().unwrap();
        let campaign = profile["campaign_package_sha256"].as_str().unwrap();
        let selection = CampaignSelection::ordinary(level).unwrap();
        assert!(CampaignSoundBank::load(&root, selection, "wrong-owner", campaign).is_err());
        if level == 1 {
            assert!(CampaignSoundBank::load(&root, selection, owner, "wrong-campaign").is_err());
            assert!(
                CampaignSoundBank::load(
                    &root,
                    CampaignSelection::ordinary(2).unwrap(),
                    owner,
                    campaign,
                )
                .is_err()
            );
        }
        CampaignSoundBank::load_if_present(&root, selection, owner, campaign)
            .unwrap()
            .unwrap()
    }

    #[test]
    fn retained_authored_transport_audio_reaches_native_voice_commands() {
        use sim_core::{FlightEvent, PlayerCommand, Position, campaign::journal::CampaignEffect};

        let level = std::env::var("MC_TRANSPORT_LEVEL").map_or(15, |value| {
            value.parse().expect("transport level is a number")
        });
        let package = std::env::var_os("MC_TRANSPORT_PACKAGE").map_or_else(
            || {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../generated/mc-bhq6-ordinary-20261024-b/level15")
            },
            PathBuf::from,
        );
        let mut campaign =
            CampaignSession::open(package, CampaignSelection::ordinary(level).unwrap())
                .expect("same retained ordinary transport package");
        let source = campaign
            .simulation()
            .entities()
            .iter()
            .find(|row| {
                row.active && (row.class, row.model, row.action, row.life) == (10, 34, 36, 0)
            })
            .expect("actual persistent authored transport")
            .clone();
        let contact = Position {
            y: source.position.y.wrapping_add(1),
            ..source.position
        };
        let packet = source.record_bytes();
        assert_ne!(
            source.control_flags & 2,
            0,
            "the retained authored transport already ran its startup sound"
        );
        let destination = (
            u16::from_le_bytes([packet[158], packet[159]]),
            u16::from_le_bytes([packet[160], packet[161]]),
        );
        // Declared local-player contact and facing; the transport, all other
        // owner rows, source lifetime and RNG remain unchanged.
        {
            let sim = campaign.simulation_mut();
            let player = sim.state.entity_index;
            sim.set_level2_entity_position(player, contact).unwrap();
            // The source is exactly one unit north of contact: source yaw0.
            sim.state.yaw = 0;
            sim.state.actual_speed = 0;
            sim.state.target_speed = 0;
        }
        let cursor = campaign.effect_cursor();
        let generation = 77;
        let mut audio = CampaignRuntimeAudio::default();
        audio.reset(&campaign, generation);
        campaign.step(PlayerCommand::default(), 0);
        assert!(
            campaign
                .simulation()
                .dispatch_events()
                .iter()
                .all(|event| !matches!(
                    event,
                    sim_core::native_world::Mc2DispatchEvent::Error { .. }
                )),
            "{:?}",
            campaign.simulation().dispatch_events()
        );
        let position = campaign.simulation().state.position;
        let near = |a: u16, b: u16| i32::from(a.wrapping_sub(b) as i16).abs() < 256;
        assert!(
            near(position.x, destination.0) && near(position.y, destination.1),
            "actual scheduled transport moved the canonical player"
        );
        let page = campaign.effects_after(cursor, 16_384).unwrap();
        let transport = page
            .records
            .iter()
            .filter(|record| {
                matches!(record.effect,
                    CampaignEffect::Flight(FlightEvent::PrepareSound {
                        entity_index, player_index: -1, sound_id: 21 | 22, ..
                    }) if entity_index == source.index
                )
            })
            .collect::<Vec<_>>();
        let sounds = transport
            .iter()
            .map(|record| match record.effect {
                CampaignEffect::Flight(FlightEvent::PrepareSound { sound_id, .. }) => sound_id,
                _ => unreachable!("retained preparation records"),
            })
            .collect::<Vec<_>>();
        assert_eq!(sounds, [22], "ordinary local arrival without replaying startup");

        let before_audio = campaign.state_hash();
        audio.sync(&campaign, generation, false).unwrap();
        assert_eq!(
            campaign.state_hash(),
            before_audio,
            "audio does not mutate gameplay"
        );
        for record in transport {
            let prepared = record.audio.expect("source-time prepared Flight audio");
            assert!(prepared.command.audible);
            let sequence = record.sequence.wrapping_add(1);
            let command = &audio.commands[&sequence];
            assert_eq!(command.generation, generation);
            assert_eq!(command.event_sequence, sequence);
            assert_eq!(command.state_hash, before_audio);
            assert_eq!(command.entity_index, source.index);
            assert_eq!(command.player_index, -1);
            assert_eq!(command.sound_flags, source.source_id as i16);
            assert_eq!(command.play_type, 1);
            assert_eq!(command.rate_delta_percent, 0);
            assert_eq!(command.playback_rate_hz, 22_050);
            assert_eq!(command.loop_count, 0);
            assert_eq!(
                (command.relative_x, command.relative_y, command.relative_z),
                (
                    prepared.command.relative_x,
                    prepared.command.relative_y,
                    prepared.command.relative_z,
                )
            );
            assert_eq!(
                (command.volume_q15, command.pan_q16),
                (prepared.command.volume_q15, prepared.command.pan_q16,)
            );
            assert_eq!(
                command.action, MC_AUDIO_ACTION_START,
                "actual unfiltered native audio drain admits the transport voice"
            );
            assert_ne!(command.voice_index, MC_AUDIO_VOICE_NONE);
            let voice = audio.voices[usize::from(command.voice_index)].unwrap();
            assert_eq!(
                (voice.sound_id, voice.sound_flags, voice.event_sequence),
                (command.sound_id, command.sound_flags, sequence,)
            );
        }
        let commands = audio.commands.clone();
        audio.sync(&campaign, generation, false).unwrap();
        assert_eq!(
            audio.commands, commands,
            "the journal cursor prevents duplicate native commands"
        );
    }

    #[test]
    fn real_selected_banks_suppress_null_without_consuming_voice() {
        let night = actual_bank("mc-oi68-level1-fullbank-v2-20261029", 1);
        let day = actual_bank("mc-oi68-level2-fullbank-v2-20261029", 2);
        let cave = actual_bank("mc-oi62-environment-level4-audio-v3", 4);
        let legacy_day = actual_bank("level3-day-spell-owner-cache-audio-v4", 3);
        let legacy_material = actual_bank(
            "level3-day-spell-owner-cache-audio-v4/components/material",
            3,
        );
        assert_eq!(legacy_day, legacy_material);
        let legacy_night_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated/mc2-level1-night-cache-v2");
        let legacy_manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(legacy_night_root.join("manifest.json")).unwrap(),
        )
        .unwrap();
        let bindings = &legacy_manifest["bindings"];
        let owner = bindings["level_export_package_sha256"].as_str().unwrap();
        let campaign = bindings["campaign_package_sha256"].as_str().unwrap();
        let legacy_night = CampaignSoundBank::load_if_present(
            &legacy_night_root,
            CampaignSelection::LEVEL1,
            owner,
            campaign,
        )
        .unwrap()
        .unwrap();
        assert_eq!(night, legacy_night);
        assert!(
            CampaignSoundBank::load_if_present(
                &legacy_night_root,
                CampaignSelection::LEVEL1,
                "wrong-owner",
                campaign,
            )
            .is_err()
        );
        let no_audio = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        assert!(
            CampaignSoundBank::load_if_present(
                &no_audio,
                CampaignSelection::LEVEL1,
                "unused-owner",
                "unused-campaign",
            )
            .unwrap()
            .is_none()
        );
        assert!(night.is_null(12).unwrap());
        assert!(!day.is_null(12).unwrap());
        assert!(!legacy_day.is_null(12).unwrap());
        assert!(legacy_day.is_null(7).unwrap());
        assert!(cave.is_null(1).unwrap());
        assert!(!night.is_null(1).unwrap());
        assert!(!cave.is_null(67).unwrap());
        assert!(cave.is_null(70).is_err());

        for (bank, null_id, real_id) in [(&night, 12, 1), (&day, 7, 12), (&cave, 1, 67)] {
            let mut commands = BTreeMap::new();
            let mut voices = [None; AUDIO_VOICE_COUNT];
            let mut null = McAudioCommandV2 {
                event_sequence: 1,
                sound_id: null_id,
                play_type: 3,
                ..Default::default()
            };
            assert!(suppress_null_sample(Some(bank), &mut null).unwrap());
            commands.insert(1, null);
            super::super::audio_voice_pool::drain_pending_audio_commands(
                &mut commands,
                &mut voices,
            );
            assert_eq!(commands[&1].action, MC_AUDIO_ACTION_SUPPRESS_NULL_SAMPLE);
            assert_eq!(commands[&1].voice_index, MC_AUDIO_VOICE_NONE);
            assert!(voices.iter().all(Option::is_none));

            let mut real = McAudioCommandV2 {
                event_sequence: 2,
                sound_id: real_id,
                play_type: 3,
                ..Default::default()
            };
            assert!(!suppress_null_sample(Some(bank), &mut real).unwrap());
            commands.insert(2, real);
            super::super::audio_voice_pool::drain_pending_audio_commands(
                &mut commands,
                &mut voices,
            );
            assert_eq!(commands[&2].action, MC_AUDIO_ACTION_START);
            let index = commands[&2].voice_index;
            assert!(
                super::super::audio_voice_pool::complete_audio_voice(&mut voices, index, 1)
                    .is_err()
            );
            assert!(voices[usize::from(index)].is_some());
            super::super::audio_voice_pool::complete_audio_voice(&mut voices, index, 2).unwrap();
            assert!(voices.iter().all(Option::is_none));
        }
    }
}
