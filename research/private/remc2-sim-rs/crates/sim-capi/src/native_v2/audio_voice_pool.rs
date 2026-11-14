//! The source's ten logical audio voices, shared by retained and campaign audio.

use std::collections::BTreeMap;

use super::{
    AUDIO_VOICE_COUNT, AbiError, AbiResult, AudioVoice, MC_AUDIO_ACTION_END_LOOP,
    MC_AUDIO_ACTION_RESTART, MC_AUDIO_ACTION_START, MC_AUDIO_ACTION_SUPPRESS_EXISTING,
    MC_AUDIO_ACTION_SUPPRESS_NO_EXISTING, MC_AUDIO_ACTION_SUPPRESS_NO_VOICE,
    MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED, MC_AUDIO_ACTION_UPDATE_EXISTING,
    MC_AUDIO_VOICE_NONE, MC_STATUS_BOUNDS, McAudioCommandV2, POSITIONAL_SOUND_PLAY_IF_NOT_PLAYING,
    positional_sound_should_replace,
};

/// Flush the existing source sound pool: one strict-volume winner per sound,
/// visited by sound ID, with the original immediate ambience exceptions.
pub(super) fn drain_pending_audio_commands(
    commands: &mut BTreeMap<u64, McAudioCommandV2>,
    voices: &mut [Option<AudioVoice>; AUDIO_VOICE_COUNT],
) {
    let unresolved = commands
        .iter()
        .filter_map(|(sequence, command)| (command.action == 0).then_some((*sequence, *command)))
        .collect::<Vec<_>>();
    let pooled_flush_id = unresolved
        .iter()
        .filter(|(_, command)| !matches!(command.sound_id, 1 | 2 | 5 | 31))
        .map(|(sequence, _)| *sequence)
        .min()
        .unwrap_or(0);
    let mut pending = BTreeMap::<u16, u64>::new();
    for (sequence, command) in unresolved {
        if matches!(command.sound_id, 1 | 2 | 5 | 31) {
            let mut command = command;
            let (action, voice_index) = allocate_audio_voice(voices, command);
            command.action = if action == MC_AUDIO_ACTION_SUPPRESS_EXISTING {
                MC_AUDIO_ACTION_UPDATE_EXISTING
            } else {
                action
            };
            command.voice_index = voice_index;
            command.pooled_flush_rank = 0;
            commands.insert(sequence, command);
            continue;
        }
        if let Some(current_sequence) = pending.get(&command.sound_id).copied() {
            let current = commands[&current_sequence];
            if positional_sound_should_replace(
                i32::from(command.volume_q15),
                i32::from(current.volume_q15),
            ) {
                commands
                    .get_mut(&current_sequence)
                    .expect("pending command remains present")
                    .action = MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED;
                pending.insert(command.sound_id, sequence);
            } else {
                commands
                    .get_mut(&sequence)
                    .expect("new command remains present")
                    .action = MC_AUDIO_ACTION_SUPPRESS_PENDING_REPLACED;
            }
        } else {
            pending.insert(command.sound_id, sequence);
        }
    }
    let mut pooled_flush_rank = 1_u8;
    for sequence in pending.into_values() {
        let mut command = commands[&sequence];
        let (action, voice_index) = allocate_audio_voice(voices, command);
        command.action = action;
        command.voice_index = voice_index;
        if matches!(
            command.action,
            MC_AUDIO_ACTION_START
                | MC_AUDIO_ACTION_RESTART
                | MC_AUDIO_ACTION_UPDATE_EXISTING
                | MC_AUDIO_ACTION_END_LOOP
        ) {
            command.pooled_flush_rank = pooled_flush_rank;
            command.reserved[0] = pooled_flush_id as u32;
            command.reserved[1] = (pooled_flush_id >> 32) as u32;
            pooled_flush_rank = pooled_flush_rank
                .checked_add(1)
                .expect("ordinary sound pool has at most 70 winners");
        } else {
            command.pooled_flush_rank = 0;
        }
        commands.insert(sequence, command);
    }
}

/// Allocate exactly as the existing retained-session audio consumer does.
/// Matching includes source sound flags; restarting replaces the acknowledged
/// event identity, while an if-not-playing request leaves that identity intact.
pub(super) fn allocate_audio_voice(
    voices: &mut [Option<AudioVoice>; AUDIO_VOICE_COUNT],
    command: McAudioCommandV2,
) -> (u8, u16) {
    let matching = voices.iter().position(|voice| {
        voice.is_some_and(|voice| {
            voice.sound_id == command.sound_id && voice.sound_flags == command.sound_flags
        })
    });
    // PlayEntitySounds_6E150 case 4 uses the ordinary finite sample with
    // flags 0, centered pan and IfNotPlayingPlaySample. Its exact volume 512
    // branch instead ends the current sample immediately. Keep the existing
    // EndLoop action for this PrepareSound stop edge; unlike an EndLoop event,
    // it retires the native voice before the following sound is admitted.
    let is_finite_if_not_playing = matches!(command.sound_id, 47 | 49) && command.play_type == 4;
    if is_finite_if_not_playing && command.volume_q15 == 512 {
        return if let Some(index) = matching {
            voices[index] = None;
            (
                MC_AUDIO_ACTION_END_LOOP,
                u16::try_from(index).expect("ten voices fit u16"),
            )
        } else {
            (MC_AUDIO_ACTION_SUPPRESS_NO_EXISTING, MC_AUDIO_VOICE_NONE)
        };
    }
    // `play_type` is the queued `EntitySounds.playType_0` value for
    // PrepareEventSound events (1 = restart-or-start, 3 = if-not-playing).
    // The four direct ambience loops retain the lower PlaySample enum
    // value 2 for if-not-playing.
    let is_direct_ambience_if_not_playing = matches!(command.sound_id, 1 | 2 | 5 | 31)
        && command.play_type == POSITIONAL_SOUND_PLAY_IF_NOT_PLAYING;
    if (command.play_type == 3 || is_direct_ambience_if_not_playing || is_finite_if_not_playing)
        && let Some(index) = matching
    {
        return (
            MC_AUDIO_ACTION_SUPPRESS_EXISTING,
            u16::try_from(index).expect("ten voices fit u16"),
        );
    }
    if command.play_type == 1
        && let Some(index) = matching
    {
        voices[index] = Some(AudioVoice {
            sound_id: command.sound_id,
            sound_flags: command.sound_flags,
            event_sequence: command.event_sequence,
        });
        return (
            MC_AUDIO_ACTION_RESTART,
            u16::try_from(index).expect("ten voices fit u16"),
        );
    }
    let Some(index) = voices.iter().position(Option::is_none) else {
        return (MC_AUDIO_ACTION_SUPPRESS_NO_VOICE, MC_AUDIO_VOICE_NONE);
    };
    voices[index] = Some(AudioVoice {
        sound_id: command.sound_id,
        sound_flags: command.sound_flags,
        event_sequence: command.event_sequence,
    });
    (
        MC_AUDIO_ACTION_START,
        u16::try_from(index).expect("ten voices fit u16"),
    )
}

/// Complete only the exact event currently owning this voice. Generation
/// validation stays with the calling session, as in the existing native ABI.
pub(super) fn complete_audio_voice(
    voices: &mut [Option<AudioVoice>; AUDIO_VOICE_COUNT],
    voice_index: u16,
    event_sequence: u64,
) -> AbiResult<()> {
    let index = usize::from(voice_index);
    let voice = voices.get_mut(index).ok_or_else(|| {
        AbiError::new(
            MC_STATUS_BOUNDS,
            format!("audio voice {voice_index} is out of bounds"),
        )
    })?;
    if !voice.is_some_and(|voice| voice.event_sequence == event_sequence) {
        return Err(AbiError::new(
            MC_STATUS_BOUNDS,
            format!("audio voice {voice_index} does not own event {event_sequence}"),
        ));
    }
    *voice = None;
    Ok(())
}
