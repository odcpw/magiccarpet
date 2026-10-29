//! Instance-owned operational preferences. CONFIG.DAT's ten assignments and
//! 0..127 volumes are retained semantically; owner game files are never edited.
//! Source: sub_5BCC0_set_any_variables1, WriteConfigDat_81DB0, and
//! sub_1A7A0_fly_asistant (49 unchanged observations, opcode 39 argument zero).

use serde::{Deserialize, Serialize};
#[path = "installed_locale.rs"]
pub mod installed_locale;
use installed_locale::{InstalledLanguages, InstalledLocaleBinding, LocaleSnapshot, LocaleSummary};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    sync::Arc,
};

pub const PREFERENCES_SCHEMA: &str = "mc2-player-preferences/v1";
pub const MAX_PREFERENCES_BYTES: u64 = 16 * 1024;

/// Portable physical key names, deliberately independent of Unity numeric IDs.
/// Escape, pause, restart, quit and save remain reserved recovery controls.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum InputKey {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    Q,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    UpArrow,
    DownArrow,
    LeftArrow,
    RightArrow,
    Return,
    Space,
    LeftControl,
    RightControl,
    LeftShift,
    RightShift,
    LeftAlt,
    RightAlt,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputDevice {
    MouseKeyboard,
    Keyboard,
    Joystick,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerPreferences {
    pub schema: String,
    pub player_name: String,
    /// Installed locale identity slot. Resource validation belongs to mc-5r5.5.
    /// None means the installed default; a filename prefix is not an identity.
    #[serde(default, deserialize_with = "optional_locale")]
    pub selected_locale: Option<String>,
    /// Source slots: forward/back/left/right, map, spell menu, player info,
    /// spell marking, next marked spell, previous marked spell.
    pub keys: [InputKey; 10],
    pub input_device: InputDevice,
    pub invert_mouse: bool,
    pub mouse_sensitivity: u16,
    pub joystick_dead_zone: u8,
    pub sound_enabled: bool,
    pub music_enabled: bool,
    pub speech_enabled: bool,
    /// Source portability option, independent of the ordinary help toggle.
    /// Older profiles omit it; port_sdl_sound.cpp initializes it to false.
    #[serde(default)]
    pub auto_show_objectives_for_foreign_languages: bool,
    pub sound_volume: u8,
    pub music_volume: u8,
    pub show_help: bool,
    /// Original graphics options; old profiles retain source enabled defaults.
    #[serde(default = "source_graphics_enabled")]
    pub reflections: bool,
    #[serde(default = "source_graphics_enabled")]
    pub sky: bool,
    #[serde(default = "source_graphics_enabled")]
    pub shadows: bool,
    pub brightness: [u8; 3],
    pub flight_assistance: bool,
    pub fullscreen: bool,
}

impl Default for PlayerPreferences {
    fn default() -> Self {
        use InputKey::*;
        Self {
            schema: PREFERENCES_SCHEMA.into(),
            player_name: "PLAYER".into(),
            selected_locale: None,
            keys: [
                UpArrow,
                DownArrow,
                LeftArrow,
                RightArrow,
                Return,
                LeftControl,
                LeftAlt,
                RightShift,
                RightShift,
                LeftAlt,
            ],
            input_device: InputDevice::MouseKeyboard,
            invert_mouse: false,
            mouse_sensitivity: 100,
            joystick_dead_zone: 15,
            sound_enabled: true,
            music_enabled: true,
            speech_enabled: true,
            auto_show_objectives_for_foreign_languages: false,
            sound_volume: 127,
            music_volume: 127,
            show_help: true,
            reflections: true,
            sky: true,
            shadows: true,
            brightness: [0; 3],
            flight_assistance: false,
            fullscreen: false,
        }
    }
}

const fn source_graphics_enabled() -> bool {
    true
}

impl PlayerPreferences {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != PREFERENCES_SCHEMA {
            return Err("unsupported preferences schema".into());
        }
        if self.player_name.is_empty()
            || self.player_name.len() > 12
            || !self
                .player_name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b' ' || b == b'-' || b == b'_')
        {
            return Err(
                "player name must contain 1..12 letters, digits, spaces, '-' or '_'".into(),
            );
        }
        if self.sound_volume > 127
            || self.music_volume > 127
            || self.brightness.iter().any(|v| *v > 4)
            || !(10..=400).contains(&self.mouse_sensitivity)
            || self.joystick_dead_zone > 75
        {
            return Err("preference value outside its supported range".into());
        }
        for i in 0..4 {
            if matches!(self.keys[i], InputKey::C | InputKey::L | InputKey::Space) {
                return Err(
                    "flight key conflicts with castle removal, recenter or quick cast".into(),
                );
            }
            if self.keys[i + 1..].contains(&self.keys[i]) {
                return Err("flight keys must be distinct from other assignments".into());
            }
        }
        if self.selected_locale.as_ref().is_some_and(|s| {
            s.is_empty()
                || s.len() > 128
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        }) {
            return Err("invalid installed locale identity".into());
        }
        Ok(())
    }
}

fn optional_locale<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.filter(|s| !s.is_empty()))
}

#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum PreferencesCommand {
    Install {
        profile_dir: String,
    },
    InstallLanguages {
        cache_dir: String,
    },
    Apply {
        preferences: PlayerPreferences,
    },
    PreviewAudio {
        sound_enabled: bool,
        music_enabled: bool,
        speech_enabled: bool,
        sound_volume: u8,
        music_volume: u8,
    },
    Cancel,
}

#[derive(Clone, Debug)]
struct AudioPreview {
    sound_enabled: bool,
    music_enabled: bool,
    speech_enabled: bool,
    sound_volume: u8,
    music_volume: u8,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PreferencesSnapshot {
    pub preferences: PlayerPreferences,
    pub installed: bool,
    pub load_status: String,
    pub assistance_sequence: u64,
    pub installed_locales: Vec<LocaleSummary>,
    pub locale_binding: Option<LocaleSnapshot>,
    pub language_status: String,
}

#[derive(Clone, Debug, Default)]
pub struct PreferenceStore {
    current: PlayerPreferences,
    path: Option<PathBuf>,
    load_status: String,
    assistance: FlightAssistance,
    assistance_sequence: u64,
    input_generation: Option<(u64, u64)>,
    audio_preview: Option<AudioPreview>,
    languages: Option<Arc<InstalledLanguages>>,
    locale: Option<Arc<InstalledLocaleBinding>>,
    language_status: String,
}

impl PreferenceStore {
    fn show_foreign_objective_text(&self) -> bool {
        if !self.current.auto_show_objectives_for_foreign_languages {
            return false;
        }
        let Some(binding) = &self.locale else {
            return false;
        };
        self.languages
            .as_ref()
            .and_then(|languages| {
                languages
                    .summaries
                    .iter()
                    .find(|summary| summary.id == binding.identity.id)
            })
            .and_then(|summary| installed_source_language_index(&summary.aliases))
            .is_some_and(|index| index != 2)
    }

    #[must_use]
    pub fn snapshot(&self) -> PreferencesSnapshot {
        let mut preferences = self.current.clone();
        if let Some(preview) = &self.audio_preview {
            preferences.sound_enabled = preview.sound_enabled;
            preferences.music_enabled = preview.music_enabled;
            preferences.speech_enabled = preview.speech_enabled;
            preferences.sound_volume = preview.sound_volume;
            preferences.music_volume = preview.music_volume;
        }
        PreferencesSnapshot {
            preferences,
            installed: self.path.is_some(),
            load_status: self.load_status.clone(),
            assistance_sequence: self.assistance_sequence,
            installed_locales: self
                .languages
                .as_ref()
                .map_or_else(Vec::new, |languages| languages.summaries.clone()),
            locale_binding: self.locale.as_ref().map(|locale| locale.identity.clone()),
            language_status: self.language_status.clone(),
        }
    }

    pub fn command(&mut self, command: PreferencesCommand) -> Result<PreferencesSnapshot, String> {
        match command {
            PreferencesCommand::Install { profile_dir } => {
                if self.path.is_some() {
                    return Err("preferences profile is already installed".into());
                }
                if profile_dir.is_empty() {
                    return Err("preferences profile directory is empty".into());
                }
                let path = PathBuf::from(profile_dir).join("player-preferences.json");
                let loaded = (|| -> Result<PlayerPreferences, String> {
                    let file = File::open(&path).map_err(|e| e.to_string())?;
                    let mut bytes = Vec::new();
                    file.take(MAX_PREFERENCES_BYTES + 1)
                        .read_to_end(&mut bytes)
                        .map_err(|e| e.to_string())?;
                    if bytes.len() as u64 > MAX_PREFERENCES_BYTES {
                        return Err("preferences file is too large".into());
                    }
                    let value: PlayerPreferences =
                        serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                    value.validate()?;
                    Ok(value)
                })();
                self.load_status = match loaded {
                    Ok(value) => {
                        self.current = value;
                        "loaded".into()
                    }
                    Err(error) => format!("defaults: {error}"),
                };
                // Recovery is read-only. Only an explicit successful Apply can
                // publish preferences; neither missing nor corrupt data touches saves.
                self.path = Some(path);
            }
            PreferencesCommand::InstallLanguages { cache_dir } => {
                let candidate = InstalledLanguages::load(&cache_dir)?;
                let (locale, recovered) =
                    match candidate.binding(self.current.selected_locale.as_deref()) {
                        Ok(locale) => (locale, false),
                        Err(_) => (candidate.binding(None)?, true),
                    };
                if recovered {
                    self.current.selected_locale = None;
                }
                self.language_status = if recovered {
                    "selected locale unavailable; using validated installed default".into()
                } else {
                    "installed source text and fonts bound".into()
                };
                self.languages = Some(Arc::new(candidate));
                self.locale = Some(locale);
            }
            PreferencesCommand::Apply { mut preferences } => {
                preferences.player_name.make_ascii_uppercase();
                preferences.validate()?;
                let path = self
                    .path
                    .as_ref()
                    .ok_or("preferences profile is not installed")?;
                // Revalidate the complete candidate before either preferences
                // publication or resource replacement. Failed reads retain the
                // old in-memory text/fonts and leave the profile untouched.
                let language_candidate = self
                    .languages
                    .as_ref()
                    .map(|languages| InstalledLanguages::load(&languages.root))
                    .transpose()?;
                let locale = language_candidate
                    .as_ref()
                    .map(|languages| languages.binding(preferences.selected_locale.as_deref()))
                    .transpose()?;
                if language_candidate.is_none()
                    && preferences.selected_locale != self.current.selected_locale
                {
                    return Err(
                        "locale selection requires the installed language resource owner".into(),
                    );
                }
                let parent = path.parent().ok_or("preferences path has no parent")?;
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                let bytes = serde_json::to_vec_pretty(&preferences).map_err(|e| e.to_string())?;
                let mut candidate = None;
                for suffix in 0..128 {
                    let temp = parent.join(format!(
                        ".player-preferences-{}-{suffix}.tmp",
                        std::process::id()
                    ));
                    match OpenOptions::new().write(true).create_new(true).open(&temp) {
                        Ok(file) => {
                            candidate = Some((temp, file));
                            break;
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                        Err(e) => return Err(e.to_string()),
                    }
                }
                let (temp, mut file) =
                    candidate.ok_or("cannot allocate preferences publication file")?;
                file.write_all(&bytes)
                    .and_then(|()| file.sync_all())
                    .map_err(|e| e.to_string())?;
                fs::rename(&temp, path).map_err(|e| e.to_string())?;
                self.current = preferences;
                if let Some(candidate) = language_candidate {
                    self.languages = Some(Arc::new(candidate));
                    self.locale = locale;
                    self.language_status = "selected source text and fonts bound".into();
                }
                self.audio_preview = None;
                self.load_status = "saved".into();
                self.assistance.reset();
            }
            PreferencesCommand::PreviewAudio {
                sound_enabled,
                music_enabled,
                speech_enabled,
                sound_volume,
                music_volume,
            } => {
                if sound_volume > 127 || music_volume > 127 {
                    return Err("audio preview volumes must be in 0..127".into());
                }
                self.audio_preview = Some(AudioPreview {
                    sound_enabled,
                    music_enabled,
                    speech_enabled,
                    sound_volume,
                    music_volume,
                });
            }
            PreferencesCommand::Cancel => {
                self.audio_preview = None;
            }
        }
        Ok(self.snapshot())
    }

    /// Called once per accepted ordinary input observation, before the command
    /// is recorded/stepped. The caller supplies authoritative mode exclusions.
    pub fn observe_input(&mut self, input: AssistanceInput, policy: AssistancePolicy) -> bool {
        let emit = self
            .assistance
            .observe(input, self.current.flight_assistance, policy);
        if emit {
            self.assistance_sequence = self.assistance_sequence.wrapping_add(1);
        }
        emit
    }

    pub fn reset_input(&mut self) {
        self.assistance.reset();
        self.input_generation = None;
    }
}

/// `LoadLanguageFile` returns the numeric owner filename suffix, not a locale
/// label. Its ordinary default selects D*.TXT before L*.TXT. Identical owner
/// files share one installed binding, so retain that source precedence within
/// its aliases; an ambiguous binding cannot establish SelectedLangIndex.
fn installed_source_language_index(aliases: &[String]) -> Option<u8> {
    for prefix in ['D', 'L'] {
        let mut selected = None;
        for alias in aliases {
            let upper = alias.to_ascii_uppercase();
            let Some(number) = upper
                .strip_prefix("LANGUAGE/")
                .and_then(|name| name.strip_prefix(prefix))
                .and_then(|name| name.strip_suffix(".TXT"))
                .filter(|number| {
                    !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
                })
            else {
                continue;
            };
            // SelectedLangIndex is a source byte; valid owner numeric names
            // have the same truncation as LoadLanguageFile's return value.
            let index = number.parse::<u32>().ok()? as u8;
            if selected.is_some_and(|old| old != index) {
                return None;
            }
            selected = Some(index);
        }
        if selected.is_some() {
            return selected;
        }
    }
    None
}

impl<B: super::GameSessionBackend> super::GameRuntime<B> {
    /// Resolved source `autoShowObjectivesForForeignLanguages &&
    /// SelectedLangIndex != 2`, using this runtime's validated locale binding.
    #[must_use]
    pub fn show_foreign_objective_text(&self) -> bool {
        self.player_preferences.show_foreign_objective_text()
    }

    /// Retained resource identity and bytes for presentation consumers on this
    /// runtime only. No process-global current language or mutable font bank.
    #[must_use]
    pub fn selected_locale_binding(&self) -> Option<Arc<InstalledLocaleBinding>> {
        self.player_preferences.locale.as_ref().map(Arc::clone)
    }
    /// Preferences remain editable only at a frontend boundary. Apply does
    /// not mutate a gameplay generation, its definition or any replay/save.
    pub fn preferences_command(
        &mut self,
        guard: super::RuntimeGuard,
        command: PreferencesCommand,
    ) -> Result<PreferencesSnapshot, String> {
        self.validate_guard(guard).map_err(|e| e.to_string())?;
        let initial_install = matches!(&command, PreferencesCommand::Install { .. })
            && self.player_preferences.path.is_none()
            && self
                .session()
                .is_none_or(|session| session.identity().tick == 0);
        if !initial_install
            && !matches!(
                self.identity().mode,
                super::RuntimeMode::Title
                    | super::RuntimeMode::Paused
                    | super::RuntimeMode::Campaign
            )
        {
            return Err("preferences require the title, campaign or paused menu".into());
        }
        self.player_preferences.command(command)
    }

    pub fn reset_preference_input(&mut self) {
        self.player_preferences.reset_input();
    }
}

#[derive(Debug)]
pub enum PreferenceStepError<E> {
    Unavailable,
    Session(super::GameSessionError<E>),
}

impl<B: super::GameSessionBackend<Command = sim_session::GameCommand>> super::GameRuntime<B> {
    /// Input assistance precedes the ordinary recorded command. Rejected
    /// commands (including stale guards or unavailable spells) do not consume
    /// observations or publish an assistance event.
    pub fn step_player_with_preferences(
        &mut self,
        guard: sim_session::SessionGuard,
        mut command: sim_session::GameCommand,
    ) -> Result<sim_session::SessionObservation<B::Snapshot>, PreferenceStepError<B::Error>> {
        if self.identity().mode != super::RuntimeMode::Playing {
            self.player_preferences.reset_input();
            return Err(PreferenceStepError::Unavailable);
        }
        let previous_assistance = self.player_preferences.assistance.clone();
        let previous_sequence = self.player_preferences.assistance_sequence;
        let previous_generation = self.player_preferences.input_generation;
        // Pause/resume, restart and mission changes advance authoritative
        // lifecycle generations even when gameplay ticks remain unchanged.
        // Their first accepted observation cannot inherit an earlier idle run.
        let input_generation = (self.generation, guard.generation);
        if previous_generation != Some(input_generation) {
            self.player_preferences.assistance.reset();
            self.player_preferences.input_generation = Some(input_generation);
        }
        let action = command_has_player_activity(&command);
        // Read the menu state retained by the gameplay backend before this
        // command is dispatched. Client/UI menu state is only an input source.
        let menu_open = self
            .session()
            .is_some_and(|session| session.backend().flight_assistance_menu_open());
        if self.player_preferences.observe_input(
            AssistanceInput {
                roll: command.roll_axis,
                pitch: command.pitch_axis,
                action,
            },
            AssistancePolicy {
                playing: true,
                menu_open,
                network: false,
            },
        ) {
            command.recenter = true;
        }
        let result = self
            .session_mut()
            .ok_or(PreferenceStepError::Unavailable)?
            .step(guard, command)
            .map_err(PreferenceStepError::Session);
        if result.is_err() {
            self.player_preferences.assistance = previous_assistance;
            self.player_preferences.assistance_sequence = previous_sequence;
            self.player_preferences.input_generation = previous_generation;
        }
        result
    }
}

/// Semantic actions are real input even when they do not carry movement or
/// cast flags. A rejected step rolls the assistance state back in the caller.
fn command_has_player_activity(command: &sim_session::GameCommand) -> bool {
    command.longitudinal != sim_session::LongitudinalControl::Coast
        || command.lateral != sim_session::LateralControl::Neutral
        || command.cast_left
        || command.cast_right
        || command.cast_selected
        || command.select_spell.is_some()
        || command.assign_spell.is_some()
        || command.mark_spell.is_some()
        || command.cycle_marked_spell.is_some()
        || command.abandon_action.is_some()
        || command.recover_player
        || command.remove_castle_stage
        || command.look_yaw != 0
        || command.look_pitch != 0
        || command.recenter
        || command.full_stop
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AssistanceInput {
    pub roll: i8,
    pub pitch: i8,
    /// Any held movement/cast or ordered action prevents the idle producer.
    pub action: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AssistancePolicy {
    pub playing: bool,
    pub menu_open: bool,
    pub network: bool,
}

#[derive(Clone, Debug, Default)]
pub struct FlightAssistance {
    previous: (i8, i8),
    unchanged: u16,
}
impl FlightAssistance {
    pub fn reset(&mut self) {
        self.previous = (0, 0);
        self.unchanged = 0;
    }

    /// Preserve source u16 counter width and strict > 0x30 threshold. Activity
    /// starts a new observation run; excluded modes cannot accumulate a stop.
    pub fn observe(
        &mut self,
        input: AssistanceInput,
        enabled: bool,
        policy: AssistancePolicy,
    ) -> bool {
        if !enabled || !policy.playing || policy.menu_open || policy.network {
            self.reset();
            return false;
        }
        let position = (input.roll, input.pitch);
        if input.action || self.previous != position {
            self.previous = position;
            self.unchanged = 0;
            return false;
        }
        self.previous = position;
        self.unchanged = self.unchanged.wrapping_add(1);
        if self.unchanged <= 0x30 {
            return false;
        }
        self.unchanged = 0;
        // Full-stop's recenter changes the position carrier for the next turn.
        self.previous = (0, 0);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_render_options_default_enabled_for_existing_profiles() {
        let mut old = serde_json::to_value(PlayerPreferences::default()).unwrap();
        for field in ["reflections", "sky", "shadows"] {
            old.as_object_mut().unwrap().remove(field);
        }
        let restored: PlayerPreferences = serde_json::from_value(old).unwrap();
        assert!(restored.reflections && restored.sky && restored.shadows);
    }

    #[test]
    fn foreign_objective_option_defaults_false_and_uses_bound_source_aliases() {
        let mut legacy = serde_json::to_value(PlayerPreferences::default()).unwrap();
        legacy
            .as_object_mut()
            .unwrap()
            .remove("auto_show_objectives_for_foreign_languages");
        let preferences: PlayerPreferences = serde_json::from_value(legacy).unwrap();
        assert!(!preferences.auto_show_objectives_for_foreign_languages);
        let mut store = PreferenceStore::default();
        store.current.auto_show_objectives_for_foreign_languages = true;
        assert!(
            !store.show_foreign_objective_text(),
            "no locale is not a foreign binding"
        );

        for (aliases, expected) in [
            (vec!["LANGUAGE/D2.TXT", "LANGUAGE/L1.TXT"], Some(2)),
            (vec!["LANGUAGE/L3.TXT"], Some(3)),
            (vec!["language/d4.txt", "LANGUAGE/L1.TXT"], Some(4)),
            (vec!["LANGUAGE/L258.TXT"], Some(2)),
            (vec!["LANGUAGE/D2.TXT", "LANGUAGE/D3.TXT"], None),
            (vec!["LANGUAGE/L1.TXT", "LANGUAGE/L3.TXT"], None),
            (vec!["opaque-locale-id", "LANGUAGE/L.TXT"], None),
        ] {
            let aliases = aliases.into_iter().map(String::from).collect::<Vec<_>>();
            assert_eq!(installed_source_language_index(&aliases), expected);
        }
    }

    #[test]
    fn installed_locale_binds_actual_text_fonts_and_objective_after_cold_profile_reopen() {
        let cache = std::env::var("MC_LANGUAGE_CACHE")
            .expect("set MC_LANGUAGE_CACHE to an actual built installed-language cache");
        let languages =
            InstalledLanguages::load(&cache).expect("complete owner language resources");
        let binding = languages.binding(None).unwrap();
        assert_eq!(languages.summaries.len(), 1);
        assert_eq!(
            languages.summaries[0].aliases,
            ["LANGUAGE/D2.TXT", "LANGUAGE/L1.TXT"]
        );
        assert_eq!(binding.records[48].as_ref(), b"\"Fly towards my beacon.\"");
        let private = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let cache_root = crate::native_world::resolve_level1_presentation_cache_root(&private);
        let mut overlay = crate::indexed_overlay::IndexedOverlayAssets::load(cache_root).unwrap();
        overlay.bind_locale(&binding).unwrap();
        assert_eq!(overlay.localization(48), Some(binding.records[48].as_ref()));
        assert_eq!(overlay.locale_binding(), Some(&binding.identity));
        let time_tag = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let profile = std::env::temp_dir().join(format!(
            "mc-locale-profile-{}-{time_tag}",
            std::process::id()
        ));
        fs::create_dir(&profile).unwrap();
        let install = || PreferencesCommand::Install {
            profile_dir: profile.to_string_lossy().into_owned(),
        };
        let isolated = profile.join("languages");
        fs::create_dir(&isolated).unwrap();
        let manifest_bytes = fs::read(std::path::Path::new(&cache).join("manifest.json")).unwrap();
        let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes).unwrap();
        for row in manifest["outputs"].as_array().unwrap() {
            let relative = row["path"].as_str().unwrap();
            let destination = isolated.join(relative);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::copy(std::path::Path::new(&cache).join(relative), destination).unwrap();
        }
        fs::write(isolated.join("manifest.json"), manifest_bytes).unwrap();
        let resources = || PreferencesCommand::InstallLanguages {
            cache_dir: isolated.to_string_lossy().into_owned(),
        };
        let binding = InstalledLanguages::load(&isolated)
            .unwrap()
            .binding(None)
            .unwrap();
        let mut store = PreferenceStore::default();
        store.command(install()).unwrap();
        store.command(resources()).unwrap();
        let mut preferences = store.snapshot().preferences;
        preferences.selected_locale = Some(binding.identity.id.clone());
        preferences.auto_show_objectives_for_foreign_languages = true;
        store
            .command(PreferencesCommand::Apply { preferences })
            .unwrap();
        let saved = fs::read(profile.join("player-preferences.json")).unwrap();
        assert!(
            !store.show_foreign_objective_text(),
            "validated D2 source binding is index 2"
        );
        let before = store.snapshot();
        let mut invalid = before.preferences.clone();
        invalid.selected_locale = Some("uninstalled-source-locale".into());
        assert!(
            store
                .command(PreferencesCommand::Apply {
                    preferences: invalid
                })
                .is_err()
        );
        assert_eq!(store.snapshot(), before);
        assert_eq!(
            fs::read(profile.join("player-preferences.json")).unwrap(),
            saved
        );
        let mut cold = PreferenceStore::default();
        cold.command(install()).unwrap();
        let reloaded = cold.command(resources()).unwrap();
        assert_eq!(reloaded.locale_binding, Some(binding.identity.clone()));
        assert!(
            reloaded
                .preferences
                .auto_show_objectives_for_foreign_languages
        );
        assert!(!cold.show_foreign_objective_text());
        let mut cold_overlay = crate::indexed_overlay::IndexedOverlayAssets::load(
            crate::native_world::resolve_level1_presentation_cache_root(&private),
        )
        .unwrap();
        cold_overlay
            .bind_locale(cold.locale.as_ref().unwrap())
            .unwrap();
        assert_eq!(
            cold_overlay.localization(23),
            Some(binding.records[23].as_ref())
        );
        fs::remove_file(isolated.join("fonts/font1.masks.u8")).unwrap();
        let retained = cold.snapshot();
        assert!(cold.command(resources()).is_err());
        assert!(
            cold.command(PreferencesCommand::Apply {
                preferences: retained.preferences.clone()
            })
            .is_err()
        );
        assert_eq!(cold.snapshot(), retained);
        assert_eq!(
            fs::read(profile.join("player-preferences.json")).unwrap(),
            saved
        );
        assert_eq!(
            cold_overlay.localization(48),
            Some(binding.records[48].as_ref())
        );
        // This is retained-resource/consumer coverage, not process/player proof.
        fs::remove_dir_all(profile).unwrap();
    }

    #[test]
    fn idle_assistance_cadence_activity_toggle_and_exclusions() {
        let mut assist = FlightAssistance::default();
        let policy = AssistancePolicy {
            playing: true,
            ..AssistancePolicy::default()
        };
        let idle = AssistanceInput::default();
        for _ in 0..48 {
            assert!(!assist.observe(idle, true, policy));
        }
        assert!(assist.observe(idle, true, policy));
        for _ in 0..48 {
            assert!(!assist.observe(idle, true, policy));
        }
        assert!(!assist.observe(
            AssistanceInput {
                action: true,
                ..idle
            },
            true,
            policy
        ));
        for _ in 0..48 {
            assert!(!assist.observe(idle, true, policy));
        }
        assert!(assist.observe(idle, true, policy));
        for excluded in [
            AssistancePolicy::default(),
            AssistancePolicy {
                menu_open: true,
                ..policy
            },
            AssistancePolicy {
                network: true,
                ..policy
            },
        ] {
            for _ in 0..100 {
                assert!(!assist.observe(idle, true, excluded));
            }
            for _ in 0..48 {
                assert!(!assist.observe(idle, true, policy));
            }
            assert!(assist.observe(idle, true, policy));
        }
        for _ in 0..100 {
            assert!(!assist.observe(idle, false, policy));
        }
        for _ in 0..48 {
            assert!(!assist.observe(idle, true, policy));
        }
        assert!(assist.observe(idle, true, policy));
    }

    #[test]
    fn spell_menu_commands_reset_idle_assistance() {
        use crate::game_definition::SemanticId;
        use sim_session::{
            GameCommand, MarkedSpellCycle, SpellAssignment, SpellAssignmentHand,
            SpellCycleDirection, SpellMark, SpellMarkHand,
        };

        let id = SemanticId::parse("spell.fireball").unwrap();
        let commands = [
            GameCommand {
                assign_spell: Some(SpellAssignment {
                    spell_id: id.clone(),
                    tier: 0,
                    hand: SpellAssignmentHand::Left,
                }),
                ..GameCommand::default()
            },
            GameCommand {
                mark_spell: Some(SpellMark {
                    spell_id: id,
                    hand: SpellMarkHand::Left,
                }),
                ..GameCommand::default()
            },
            GameCommand {
                cycle_marked_spell: Some(MarkedSpellCycle {
                    hand: SpellAssignmentHand::Left,
                    direction: SpellCycleDirection::Next,
                }),
                ..GameCommand::default()
            },
        ];
        let policy = AssistancePolicy {
            playing: true,
            ..AssistancePolicy::default()
        };
        for command in commands {
            let mut assist = FlightAssistance::default();
            for _ in 0..48 {
                assert!(!assist.observe(AssistanceInput::default(), true, policy));
            }
            assert!(!assist.observe(
                AssistanceInput {
                    action: command_has_player_activity(&command),
                    ..AssistanceInput::default()
                },
                true,
                policy
            ));
            for _ in 0..48 {
                assert!(!assist.observe(AssistanceInput::default(), true, policy));
            }
            assert!(assist.observe(AssistanceInput::default(), true, policy));
        }
        assert!(!command_has_player_activity(&GameCommand::default()));
    }

    #[test]
    fn persistence_cancel_invalid_recovery_and_failed_apply_are_atomic() {
        // A unique directory label, never authentication material. create_dir
        // below refuses collisions instead of overwriting a prior test run.
        let time_tag = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "mc-preferences-test-{}-{time_tag}",
            std::process::id()
        ));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("player-preferences.json");
        let save = dir.join("campaign-save.json");
        fs::write(&save, b"existing campaign save").unwrap();
        let install = || PreferencesCommand::Install {
            profile_dir: dir.to_string_lossy().into_owned(),
        };
        let mut missing = PreferenceStore::default();
        assert_eq!(
            missing.command(install()).unwrap().preferences,
            PlayerPreferences::default()
        );
        assert!(!path.exists());
        fs::write(&path, b"invalid settings").unwrap();
        let mut store = PreferenceStore::default();
        assert!(
            store
                .command(install())
                .unwrap()
                .load_status
                .starts_with("defaults:")
        );
        assert_eq!(fs::read(&path).unwrap(), b"invalid settings");
        let mut remapped = PlayerPreferences::default();
        remapped.keys[..3].copy_from_slice(&[InputKey::I, InputKey::J, InputKey::K]);
        assert!(
            remapped.validate().is_ok(),
            "unreserved letters can be flight bindings"
        );
        remapped.keys[0] = InputKey::L;
        assert!(
            remapped.validate().is_err(),
            "the original castle-removal key remains reserved"
        );
        let mut prefs = PlayerPreferences::default();
        prefs.player_name = "Oliver".into();
        prefs.keys[0] = InputKey::W;
        prefs.sound_volume = 63;
        prefs.show_help = false;
        prefs.reflections = false;
        prefs.sky = false;
        prefs.shadows = false;
        prefs.auto_show_objectives_for_foreign_languages = true;
        store
            .command(PreferencesCommand::Apply { preferences: prefs })
            .unwrap();
        store.command(PreferencesCommand::Cancel).unwrap();
        let saved = fs::read(&path).unwrap();
        store
            .command(PreferencesCommand::PreviewAudio {
                sound_enabled: true,
                music_enabled: false,
                speech_enabled: false,
                sound_volume: 0,
                music_volume: 32,
            })
            .unwrap();
        assert_eq!(store.snapshot().preferences.sound_volume, 0);
        assert!(!store.snapshot().preferences.music_enabled);
        assert_eq!(fs::read(&path).unwrap(), saved);
        store.command(PreferencesCommand::Cancel).unwrap();
        assert_eq!(store.snapshot().preferences.sound_volume, 63);
        assert!(store.snapshot().preferences.music_enabled);
        let mut invalid = store.snapshot().preferences;
        invalid.sound_volume = 128;
        assert!(
            store
                .command(PreferencesCommand::Apply {
                    preferences: invalid
                })
                .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), saved);
        let mut cold = PreferenceStore::default();
        assert_eq!(
            cold.command(install()).unwrap().preferences,
            store.snapshot().preferences
        );
        assert_eq!(cold.snapshot().preferences.player_name, "OLIVER");
        assert_eq!(fs::read(&save).unwrap(), b"existing campaign save");
        fs::remove_file(path).unwrap();
        fs::remove_file(save).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
