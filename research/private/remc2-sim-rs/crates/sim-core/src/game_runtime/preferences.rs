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
    Minus,
    Equals,
    Semicolon,
    Backslash,
    Comma,
    Period,
    Slash,
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
            brightness: [0; 3],
            flight_assistance: false,
            fullscreen: false,
        }
    }
}

impl PlayerPreferences {
    pub fn validate(&self) -> Result<(), String> {
        self.validate_with_legacy_objective_key(false)
    }

    fn validate_with_legacy_objective_key(&self, allow_objective_key: bool) -> Result<(), String> {
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
        if !allow_objective_key && self.keys.contains(&InputKey::O) {
            return Err("key conflicts with the fixed objective control".into());
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

    // Old profiles could bind O despite the fixed objective control. Keep all
    // other assignments, preferring the original slot default when it is free.
    // The fallback keys are all offered by source sub_79E10, and outnumber the
    // other nine slots, so a free key always exists.
    fn repair_legacy_objective_keys(&mut self) {
        use InputKey::*;
        const FALLBACKS: [InputKey; 16] = [
            UpArrow, DownArrow, LeftArrow, RightArrow, Return, LeftControl, LeftAlt,
            RightShift, Q, W, T, Y, U, A, F, G,
        ];
        let defaults = Self::default().keys;
        for slot in 0..self.keys.len() {
            if self.keys[slot] != O {
                continue;
            }
            let replacement = std::iter::once(defaults[slot])
                .chain(FALLBACKS)
                .find(|key| !self.keys.contains(key))
                .expect("more fallback keys than other assignment slots");
            self.keys[slot] = replacement;
        }
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
    SourceGraphics {
        #[serde(deserialize_with="deserialize_brightness_guard")]
        session_guard:sim_session::SessionGuard,
        action:super::SourceGraphicsAction,
    },
    SourceHelpMode {
        #[serde(deserialize_with = "deserialize_brightness_guard")]
        session_guard: sim_session::SessionGuard,
    },
    SourceFlightAssistance {
        #[serde(deserialize_with = "deserialize_brightness_guard")]
        session_guard: sim_session::SessionGuard,
    },
    SourceBrightness {
        #[serde(deserialize_with = "deserialize_brightness_guard")]
        session_guard: sim_session::SessionGuard,
        action: SourceBrightnessAction,
    },
    SourceAudio {
        #[serde(deserialize_with = "deserialize_brightness_guard")]
        session_guard: sim_session::SessionGuard,
        action: SourceAudioAction,
        #[serde(default)]
        speech_available: Option<bool>,
    },
    Install {
        profile_dir: String,
    },
    InstallLanguages {
        cache_dir: String,
    },
    InstallLanguagesChecked {
        cache_dir: String,
        expected_locale_binding: LocaleSnapshot,
    },
    Apply {
        preferences: PlayerPreferences,
    },
    ApplyChecked {
        preferences: PlayerPreferences,
        expected_locale_binding: LocaleSnapshot,
    },
    /// SetKeysDialog result1 / source sub_5BCC0. This is an immediate
    /// committed reset of the named source fields, not a replacement draft.
    ResetDefaultKeysChecked {
        expected_locale_binding: LocaleSnapshot,
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SourceBrightnessAction { Cycle, Increment, Decrement }

/// Source ChangeSettings cases 2, 3 and 4; availability is supplied by the
/// active audio owner, independently of the user's enabled preference.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceAudioAction { Sound, Music, Speech }

fn deserialize_brightness_guard<'de, D: serde::Deserializer<'de>>(deserializer: D)
    -> Result<sim_session::SessionGuard, D::Error>
{
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Guard { session_id: String, generation: String, expected_tick: String }
    let guard = Guard::deserialize(deserializer)?;
    let parse = |value: &str| -> Result<u64, D::Error> {
        if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit())
            || (value.len() > 1 && value.starts_with('0')) {
            return Err(serde::de::Error::custom("session guard requires canonical decimal strings"));
        }
        value.parse::<u64>().map_err(serde::de::Error::custom)
    };
    Ok(sim_session::SessionGuard { session_id: parse(&guard.session_id)?, generation: parse(&guard.generation)?,
        expected_tick: parse(&guard.expected_tick)? })
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
    /// Ephemeral result consumed directly by the existing command dispatcher.
    /// It is never stored in runtime identity or preferences persistence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_brightness_admitted: Option<bool>,
    /// Ephemeral result for the guarded source audio command.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_audio_admitted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_flight_assistance_admitted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_help_mode_admitted: Option<bool>,
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
    /// SwitchHelpMode_18AA0. Pinned sub_871E0 always returns1; there is no
    /// ordinary host capability to guess from assets or audio state.
    pub fn source_help_mode_toggle(&mut self) -> Result<Option<bool>, String> {
        if self.audio_preview.is_some() { return Ok(None); }
        let mut preferences = self.current.clone();
        preferences.show_help = !preferences.show_help;
        self.write_profile(&preferences)?;
        let enabled = preferences.show_help;
        self.current = preferences;
        self.load_status = "saved".into();
        Ok(Some(enabled))
    }

    /// Source case5 changes only the operational flag. The excluded idle
    /// observations retain their sample/counter until assistance runs again.
    pub fn source_flight_assistance_toggle(&mut self) -> Result<Option<bool>, String> {
        if self.audio_preview.is_some() {
            return Ok(None);
        }
        let mut preferences = self.current.clone();
        preferences.flight_assistance = !preferences.flight_assistance;
        self.write_profile(&preferences)?;
        let enabled = preferences.flight_assistance;
        self.current = preferences;
        self.load_status = "saved".into();
        Ok(Some(enabled))
    }

    fn write_profile(&self, preferences: &PlayerPreferences) -> Result<(), String> {
        let path = self.path.as_ref().ok_or("preferences profile is not installed")?;
        let parent = path.parent().ok_or("preferences path has no parent")?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let bytes = serde_json::to_vec_pretty(preferences).map_err(|error| error.to_string())?;
        let mut candidate = None;
        for suffix in 0..128 {
            let temp = parent.join(format!(".player-preferences-{}-{suffix}.tmp", std::process::id()));
            match OpenOptions::new().write(true).create_new(true).open(&temp) {
                Ok(file) => { candidate = Some((temp, file)); break; }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.to_string()),
            }
        }
        let (temp, mut file) = candidate.ok_or("cannot allocate preferences publication file")?;
        let written = file.write_all(&bytes).and_then(|()| file.sync_all());
        drop(file);
        let published = written.and_then(|()| fs::rename(&temp, path));
        if published.is_err() {
            // This attempt owns the create_new file exclusively. Retire it so
            // repeated failures cannot exhaust the publication names.
            let _ = fs::remove_file(&temp);
        }
        published.map_err(|error| error.to_string())
    }

    /// Commit an admitted source audio toggle without disturbing other owners.
    /// An active dialog preview must retain its independent cancel semantics.
    pub fn source_audio_toggle(
        &mut self, action: SourceAudioAction, available: bool,
    ) -> Result<Option<bool>, String> {
        if !available || self.audio_preview.is_some() {
            return Ok(None);
        }
        let mut preferences = self.current.clone();
        let enabled = match action {
            SourceAudioAction::Sound => &mut preferences.sound_enabled,
            SourceAudioAction::Music => &mut preferences.music_enabled,
            SourceAudioAction::Speech => &mut preferences.speech_enabled,
        };
        *enabled = !*enabled;
        let enabled = *enabled;
        self.write_profile(&preferences)?;
        self.current = preferences;
        self.load_status = "saved".into();
        Ok(Some(enabled))
    }

    /// Borrow the authoritative name without materializing a preferences snapshot.
    pub(super) fn player_name(&self) -> &str {
        &self.current.player_name
    }

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
            source_brightness_admitted: None,
            source_audio_admitted: None,
            source_flight_assistance_admitted: None,
            source_help_mode_admitted: None,
        }
    }

    pub fn command(&mut self, command: PreferencesCommand) -> Result<PreferencesSnapshot, String> {
        match command {
            PreferencesCommand::SourceGraphics {..}=>return Err("source graphics requires a guarded active campaign".into()),
            PreferencesCommand::SourceHelpMode { .. } => return Err("source help mode requires a guarded active campaign".into()),
            PreferencesCommand::SourceFlightAssistance { .. } => return Err("source flight assistance requires a guarded active campaign".into()),
            PreferencesCommand::SourceBrightness { .. } => return Err("source brightness requires a guarded active campaign".into()),
            PreferencesCommand::SourceAudio { .. } => return Err("source audio requires a guarded active campaign".into()),
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
                    // Only stored legacy profiles may contain these obsolete
                    // GAME flags. Validate their original types, discard their
                    // authority, and preserve every unrelated strict field.
                    // Preserve serde's original duplicate-field refusal while
                    // decoding the one historical top-level profile shape.
                    struct ProfileFields(serde_json::Map<String,serde_json::Value>);
                    impl<'de> Deserialize<'de> for ProfileFields {
                        fn deserialize<D:serde::Deserializer<'de>>(decoder:D)->Result<Self,D::Error> {
                            struct FieldsVisitor;
                            impl<'input> serde::de::Visitor<'input> for FieldsVisitor {
                                type Value=ProfileFields;
                                fn expecting(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {f.write_str("a preferences object")}
                                fn visit_map<A:serde::de::MapAccess<'input>>(self,mut input:A)->Result<Self::Value,A::Error> {
                                    let mut fields=serde_json::Map::new();
                                    while let Some((key,value))=input.next_entry::<String,serde_json::Value>()? {
                                        if fields.insert(key.clone(),value).is_some() {
                                            return Err(serde::de::Error::custom(format!("duplicate preferences field {key}")));
                                        }
                                    }
                                    Ok(ProfileFields(fields))
                                }
                            }
                            decoder.deserialize_map(FieldsVisitor)
                        }
                    }
                    let ProfileFields(mut fields)=serde_json::from_slice::<ProfileFields>(&bytes).map_err(|e|e.to_string())?;
                    for name in ["reflections","shadows","sky","dynamic_lighting"] {
                        if let Some(value)=fields.remove(name) {
                            if !value.is_boolean() {return Err(format!("legacy graphics field {name} must be boolean"));}
                        }
                    }
                    let mut value:PlayerPreferences=serde_json::from_value(serde_json::Value::Object(fields)).map_err(|e|e.to_string())?;
                    // Only an otherwise-valid old profile receives this repair;
                    // invalid profiles still take the existing recovery path.
                    value.validate_with_legacy_objective_key(true)?;
                    value.repair_legacy_objective_keys();
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
            command @ (PreferencesCommand::InstallLanguages { .. }
            | PreferencesCommand::InstallLanguagesChecked { .. }) => {
                let (cache_dir, expected) = match command {
                    PreferencesCommand::InstallLanguages { cache_dir } => (cache_dir, None),
                    PreferencesCommand::InstallLanguagesChecked {
                        cache_dir,
                        expected_locale_binding,
                    } => (cache_dir, Some(expected_locale_binding)),
                    _ => unreachable!(),
                };
                let candidate = InstalledLanguages::load(&cache_dir)?;
                let (locale, recovered) =
                    match candidate.binding(self.current.selected_locale.as_deref()) {
                        Ok(locale) => (locale, false),
                        Err(_) => (candidate.binding(None)?, true),
                    };
                if expected
                    .as_ref()
                    .is_some_and(|expected| *expected != locale.identity)
                {
                    return Err("installed locale changed after preflight".into());
                }
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
            command @ (PreferencesCommand::Apply { .. }
            | PreferencesCommand::ApplyChecked { .. }
            | PreferencesCommand::ResetDefaultKeysChecked { .. }) => {
                let (mut preferences, expected) = match command {
                    PreferencesCommand::Apply { preferences } => (preferences, None),
                    PreferencesCommand::ApplyChecked {
                        preferences,
                        expected_locale_binding,
                    } => (preferences, Some(expected_locale_binding)),
                    PreferencesCommand::ResetDefaultKeysChecked { expected_locale_binding } => {
                        let mut preferences = self.current.clone();
                        preferences.keys = PlayerPreferences::default().keys;
                        preferences.sound_volume = 127;
                        preferences.music_volume = 127;
                        preferences.show_help = true;
                        preferences.brightness[0] = 0;
                        preferences.brightness[2] = 0;
                        // langIndex_4 is only the bootstrap lookup scratch;
                        // active text and WriteConfigDat use SelectedLangIndex.
                        // brightness_12, display settings and other preferences
                        // are not written by source sub_5BCC0.
                        (preferences, Some(expected_locale_binding))
                    }
                    _ => unreachable!(),
                };
                preferences.player_name.make_ascii_uppercase();
                preferences.validate()?;
                if self.path.is_none() { return Err("preferences profile is not installed".into()); }
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
                if expected.as_ref().is_some_and(|expected| {
                    locale
                        .as_ref()
                        .is_none_or(|locale| locale.identity != *expected)
                }) {
                    return Err("installed locale changed after preflight".into());
                }
                if language_candidate.is_none()
                    && preferences.selected_locale != self.current.selected_locale
                {
                    return Err(
                        "locale selection requires the installed language resource owner".into(),
                    );
                }
                self.write_profile(&preferences)?;
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

    /// Pause/resume keeps the same input owner. Only carry a matching binding;
    /// an actual reset or new session still starts a fresh observation run.
    pub(super) fn carry_input_generation(&mut self, previous: (u64, u64), next: (u64, u64)) {
        if self.input_generation == Some(previous) {
            self.input_generation = Some(next);
        }
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

impl super::GameRuntime<crate::campaign::backend::Mc2CampaignBackend> {
    /// Source case6 alone may change CONFIG brightness while Playing. Both
    /// guards and source palette readiness precede file or retained mutation.
    pub fn source_brightness_preferences(
        &mut self, guard: super::RuntimeGuard, session_guard: sim_session::SessionGuard,
        action: SourceBrightnessAction,
    ) -> Result<PreferencesSnapshot, super::RuntimeTransitionError> {
        use super::{RuntimeMode, RuntimeState, RuntimeTransitionError};
        self.validate_guard(guard).map_err(Box::new).map_err(RuntimeTransitionError::Guard)?;
        let actual = self.identity().mode;
        let RuntimeState::Playing { session, .. } = &mut self.state else {
            return Err(RuntimeTransitionError::InvalidMode { expected: RuntimeMode::Playing, actual });
        };
        session.completion_status(session_guard).map_err(Box::new).map_err(RuntimeTransitionError::SessionGuard)?;
        if !session.backend().source_brightness_admitted() {
            let mut snapshot = self.player_preferences.snapshot();
            snapshot.source_brightness_admitted = Some(false);
            return Ok(snapshot);
        }
        let index = session.backend().source_brightness_slot();
        let mut preferences = self.player_preferences.current.clone();
        let current = preferences.brightness[index];
        preferences.brightness[index] = match action {
            SourceBrightnessAction::Cycle => (current + 1) % 5,
            SourceBrightnessAction::Increment => current.saturating_add(1).min(4),
            SourceBrightnessAction::Decrement => current.saturating_sub(1),
        };
        self.player_preferences.write_profile(&preferences)
            .map_err(|message| RuntimeTransitionError::InvalidRuntimeFlow { message })?;
        // All remaining publication is infallible after the private file has
        // atomically committed. Do not reset input/audio/locale draft owners.
        session.backend_mut().request_source_brightness(preferences.brightness[index]);
        self.player_preferences.current = preferences;
        self.player_preferences.load_status = "saved".into();
        let mut snapshot = self.player_preferences.snapshot();
        snapshot.source_brightness_admitted = Some(true);
        Ok(snapshot)
    }
}

impl<B: super::GameSessionBackend<Command = sim_session::GameCommand>> super::GameRuntime<B> {
    /// Input assistance precedes the ordinary recorded command. Rejected
    /// commands (including stale guards or unavailable spells) do not consume
    /// observations or publish an assistance event.
    pub fn step_player_with_preferences(
        &mut self,
        guard: sim_session::SessionGuard,
        command: sim_session::GameCommand,
    ) -> Result<sim_session::SessionObservation<B::Snapshot>, PreferenceStepError<B::Error>> {
        self.advance_player_with_preferences(guard, command)?;
        Ok(self
            .session()
            .expect("an accepted player step retains its Playing session")
            .snapshot())
    }

    /// Advance through the same preference and rollback boundary when the
    /// transport obtains its campaign observation in a separate read.
    #[doc(hidden)]
    pub fn advance_player_with_preferences(
        &mut self,
        guard: sim_session::SessionGuard,
        mut command: sim_session::GameCommand,
    ) -> Result<(), PreferenceStepError<B::Error>> {
        if self.identity().mode != super::RuntimeMode::Playing {
            return Err(PreferenceStepError::Unavailable);
        }
        self.session().ok_or(PreferenceStepError::Unavailable)?
            .completion_status(guard)
            .map_err(|error| PreferenceStepError::Session(sim_session::GameSessionError::Guard(error)))?;
        // The exact input sample belongs to this operational producer, not
        // GAME history. Record only its derived recenter and existing axes.
        let steering_observation = command.steering_observation.take();
        let top_bar = self.display_settings.top_bar;
        let brightness = self.player_preferences.current.brightness;
        let previous_assistance = self.player_preferences.assistance.clone();
        let previous_sequence = self.player_preferences.assistance_sequence;
        let previous_generation = self.player_preferences.input_generation;
        // Real resets and mission changes start a new input owner. An admitted
        // pause/resume carries its exact generation binding without consuming
        // the source idle counter.
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
                steering_observation,
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
        let session = self.session_mut().ok_or(PreferenceStepError::Unavailable)?;
        session.backend_mut().prepare_hud_frame(top_bar);
        session.backend_mut().prepare_palette_frame(brightness);
        let result = session.advance(guard, command)
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
    pub steering_observation: Option<sim_session::SteeringObservation>,
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
    previous: sim_session::SteeringObservation,
    unchanged: u16,
}
impl FlightAssistance {
    pub fn reset(&mut self) {
        self.previous = sim_session::SteeringObservation::default();
        self.unchanged = 0;
    }

    /// Preserve source u16 counter width and strict > 0x30 threshold. Activity
    /// starts a new observation run; excluded modes retain, but cannot advance,
    /// the previous sample and counter (sub_1A7A0).
    pub fn observe(
        &mut self,
        input: AssistanceInput,
        enabled: bool,
        policy: AssistancePolicy,
    ) -> bool {
        if !enabled || !policy.playing || policy.menu_open || policy.network {
            return false;
        }
        let position = input.steering_observation.unwrap_or_else(||
            sim_session::SteeringObservation::from_axes(input.roll, input.pitch));
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
        // Opcode39 is only queued here. Retain the observed sample: the later
        // PlayerEvents handler recenters, and that changed input is observed
        // on the next pass rather than manufactured in this producer.
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_preferences_matches_observed_steps_and_rolls_back_rejections() {
        use crate::{
            campaign::backend::{
                Mc2CampaignAdapter, Mc2CampaignBackend, Mc2CampaignInputs, load_mc2_campaign,
            },
            game_definition::{SemanticId, default_catalog},
            game_runtime::GameRuntime,
        };
        use sim_session::{GameCommand, GameSessionError, SessionGuard};

        let package = std::env::var_os("MC_FLIGHT_ASSISTANCE_PACKAGE").map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../generated/mc-bhq8-level1-collision-v5-night"));
        let definition =
            Arc::new(load_mc2_campaign(&package, &default_catalog().unwrap()).unwrap());
        let open = || {
            let mut runtime = GameRuntime::<Mc2CampaignBackend>::title(7137).unwrap();
            let loading = runtime
                .new_game(runtime.identity().guard(), Arc::clone(&definition))
                .unwrap();
            runtime
                .open_level(
                    loading.guard(),
                    &Mc2CampaignAdapter,
                    Mc2CampaignInputs::new(&package),
                    7137,
                )
                .unwrap();
            let guard = runtime.session().unwrap().identity().guard();
            runtime.player_preferences.current.flight_assistance = true;
            // Put the real preference producer one accepted observation from
            // recentering, without running 48 unrelated gameplay frames.
            runtime.player_preferences.assistance.unchanged = 48;
            runtime.player_preferences.input_generation =
                Some((runtime.generation, guard.generation));
            runtime
        };
        let assistance = |runtime: &GameRuntime<Mc2CampaignBackend>| {
            let preferences = &runtime.player_preferences;
            (
                preferences.assistance.previous,
                preferences.assistance.unchanged,
                preferences.assistance_sequence,
                preferences.input_generation,
            )
        };
        let mut observed = open();
        let mut advanced = open();
        let guard = observed.session().unwrap().identity().guard();
        let before = observed.session().unwrap().snapshot();
        let before_assistance = assistance(&observed);
        for (rejected_guard, mut command, backend_rejection) in [
            (
                SessionGuard {
                    expected_tick: 1,
                    ..guard
                },
                GameCommand::default(),
                false,
            ),
            (
                SessionGuard {
                    generation: guard.generation + 1,
                    ..guard
                },
                GameCommand::default(),
                false,
            ),
            (
                guard,
                GameCommand {
                    select_spell: Some(SemanticId::parse("spell.castle").unwrap()),
                    ..GameCommand::default()
                },
                true,
            ),
        ] {
            command.steering_observation = Some(sim_session::SteeringObservation::try_from([0.45_f32.to_bits(), 0]).unwrap());
            let observed_error = observed
                .step_player_with_preferences(rejected_guard, command.clone())
                .unwrap_err();
            let advanced_error = advanced
                .advance_player_with_preferences(rejected_guard, command)
                .unwrap_err();
            assert_eq!(format!("{observed_error:?}"), format!("{advanced_error:?}"));
            if backend_rejection {
                assert!(matches!(
                    advanced_error,
                    PreferenceStepError::Session(GameSessionError::Backend(_))
                ));
            } else {
                assert!(matches!(
                    advanced_error,
                    PreferenceStepError::Session(GameSessionError::Guard(_))
                ));
            }
            for runtime in [&observed, &advanced] {
                assert_eq!(assistance(runtime), before_assistance);
                assert_eq!(runtime.session().unwrap().snapshot(), before);
                assert_eq!(
                    runtime
                        .session()
                        .unwrap()
                        .backend()
                        .replay_operation_count(),
                    0
                );
            }
        }

        let stepped = observed
            .step_player_with_preferences(guard, GameCommand::default())
            .unwrap();
        advanced
            .advance_player_with_preferences(guard, GameCommand::default())
            .unwrap();
        assert_eq!(advanced.session().unwrap().snapshot(), stepped);
        assert_eq!(assistance(&advanced), assistance(&observed));
        assert_eq!(advanced.player_preferences.assistance_sequence, 1);
        assert_eq!(advanced.player_preferences.assistance.unchanged, 0);
        for runtime in [&mut observed, &mut advanced] {
            assert_eq!(
                runtime
                    .session()
                    .unwrap()
                    .backend()
                    .replay_operation_count(),
                1
            );
            for _ in 0..48 {
                let guard = runtime.session().unwrap().identity().guard();
                runtime.advance_player_with_preferences(guard, GameCommand::default()).unwrap();
            }
            assert_eq!(runtime.player_preferences.assistance.unchanged, 48);
            runtime.pause(runtime.identity().guard()).unwrap();
        }
        let paused = advanced.session().unwrap().snapshot();
        let paused_operations = advanced
            .session()
            .unwrap()
            .backend()
            .replay_operation_count();
        let paused_guard = paused.identity.guard();
        assert!(matches!(
            observed.step_player_with_preferences(paused_guard, GameCommand::default()),
            Err(PreferenceStepError::Unavailable)
        ));
        assert!(matches!(
            advanced.advance_player_with_preferences(paused_guard, GameCommand::default()),
            Err(PreferenceStepError::Unavailable)
        ));
        for runtime in [&observed, &advanced] {
            assert_eq!(runtime.session().unwrap().snapshot(), paused);
            assert_eq!(
                runtime
                    .session()
                    .unwrap()
                    .backend()
                    .replay_operation_count(),
                paused_operations
            );
            assert_eq!(runtime.player_preferences.input_generation,
                Some((runtime.generation, paused_guard.generation)));
            assert_eq!(runtime.player_preferences.assistance.unchanged, 48,
                "paused exclusion and a rejected ordinary input retain the source idle run");
        }
        for runtime in [&mut observed, &mut advanced] {
            runtime.resume(runtime.identity().guard()).unwrap();
            let before = assistance(runtime);
            assert!(runtime.advance_player_with_preferences(paused_guard, GameCommand::default()).is_err());
            assert_eq!(assistance(runtime), before, "a stale pre-resume guard consumes nothing");
            let guard = runtime.session().unwrap().identity().guard();
            runtime.advance_player_with_preferences(guard, GameCommand::default()).unwrap();
            assert_eq!(runtime.player_preferences.assistance_sequence, 2,
                "the49th actual idle observation follows the retained pause boundary");
            assert_eq!(runtime.player_preferences.assistance.unchanged, 0);
            let session = runtime.session_mut().unwrap();
            session.reset(session.identity().guard()).unwrap();
            let guard = runtime.session().unwrap().identity().guard();
            runtime.advance_player_with_preferences(guard, GameCommand::default()).unwrap();
            assert_eq!(runtime.player_preferences.assistance.unchanged, 1,
                "a genuine session reset still starts a fresh input owner");
            assert_eq!(runtime.player_preferences.assistance_sequence, 2);
        }
    }

    #[test]
    fn source_default_keys_preserves_selected_locale_and_publishes_only_source_fields_atomically() {
        use crate::{
            campaign::backend::{Mc2CampaignAdapter, Mc2CampaignBackend, Mc2CampaignInputs, load_mc2_campaign},
            game_definition::default_catalog,
            game_runtime::{GameRuntime, RuntimeGuard},
        };
        let cache = PathBuf::from(std::env::var_os("MC_LANGUAGE_CACHE")
            .expect("set MC_LANGUAGE_CACHE to the actual installed language directory"));
        let tag = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let profile = std::env::temp_dir().join(format!("mc-source-default-keys-{}-{tag}", std::process::id()));
        fs::create_dir(&profile).unwrap();
        let isolated = profile.join("languages");
        fs::create_dir(&isolated).unwrap();
        let manifest_bytes = fs::read(cache.join("manifest.json")).unwrap();
        let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes).unwrap();
        for row in manifest["outputs"].as_array().unwrap() {
            let relative = row["path"].as_str().unwrap();
            let destination = isolated.join(relative);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::copy(cache.join(relative), destination).unwrap();
        }
        fs::write(isolated.join("manifest.json"), manifest_bytes).unwrap();
        let locale = InstalledLanguages::load(&isolated).unwrap().binding(None).unwrap();
        let expected = locale.identity.clone();
        let mut runtime = GameRuntime::<Mc2CampaignBackend>::title(8870).unwrap();
        let outer = runtime.identity().guard();
        runtime.preferences_command(outer, PreferencesCommand::Install {
            profile_dir: profile.to_string_lossy().into_owned(),
        }).unwrap();
        runtime.preferences_command(outer, PreferencesCommand::InstallLanguagesChecked {
            cache_dir: isolated.to_string_lossy().into_owned(), expected_locale_binding: expected.clone(),
        }).unwrap();
        let mut customized = PlayerPreferences::default();
        customized.player_name = "CUSTOM".into();
        customized.selected_locale = Some(expected.id.clone());
        customized.keys = [InputKey::W, InputKey::S, InputKey::A, InputKey::D,
            InputKey::Return, InputKey::LeftControl, InputKey::LeftAlt, InputKey::RightShift,
            InputKey::RightShift, InputKey::LeftAlt];
        customized.sound_volume = 17;
        customized.music_volume = 23;
        customized.show_help = false;
        customized.brightness = [4, 3, 2];
        customized.sound_enabled = false;
        customized.music_enabled = false;
        customized.speech_enabled = false;
        customized.auto_show_objectives_for_foreign_languages = true;
        customized.input_device = InputDevice::Joystick;
        customized.invert_mouse = true;
        customized.mouse_sensitivity = 137;
        customized.joystick_dead_zone = 27;
        customized.flight_assistance = true;
        customized.fullscreen = true;
        runtime.preferences_command(outer, PreferencesCommand::ApplyChecked {
            preferences: customized.clone(), expected_locale_binding: expected.clone(),
        }).unwrap();
        let path = profile.join("player-preferences.json");
        let before = runtime.identity();
        let before_bytes = fs::read(&path).unwrap();
        let mut changed_binding = expected.clone();
        changed_binding.text_sha256 = "0".repeat(64);
        assert!(runtime.preferences_command(outer, PreferencesCommand::ResetDefaultKeysChecked {
            expected_locale_binding: changed_binding,
        }).is_err());
        assert_eq!(runtime.identity(), before);
        assert_eq!(fs::read(&path).unwrap(), before_bytes);
        assert!(runtime.preferences_command(RuntimeGuard { generation: outer.generation + 1, ..outer },
            PreferencesCommand::ResetDefaultKeysChecked { expected_locale_binding: expected.clone() }).is_err());
        assert_eq!(runtime.identity(), before);
        assert_eq!(fs::read(&path).unwrap(), before_bytes);

        // Real missing output and real atomic-rename failure both retain the
        // current binding/preferences and the existing usable profile bytes.
        let output = manifest["outputs"][0]["path"].as_str().unwrap();
        let output_path = isolated.join(output);
        let retained = fs::read(&output_path).unwrap();
        fs::remove_file(&output_path).unwrap();
        assert!(runtime.preferences_command(outer, PreferencesCommand::ResetDefaultKeysChecked {
            expected_locale_binding: expected.clone(),
        }).is_err());
        assert_eq!(runtime.identity(), before);
        assert_eq!(fs::read(&path).unwrap(), before_bytes);
        fs::write(output_path, retained).unwrap();
        let retained_profile = profile.join("retained-profile.json");
        fs::rename(&path, &retained_profile).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(runtime.preferences_command(outer, PreferencesCommand::ResetDefaultKeysChecked {
            expected_locale_binding: expected.clone(),
        }).is_err());
        assert_eq!(runtime.identity(), before);
        assert_eq!(fs::read(&retained_profile).unwrap(), before_bytes);
        fs::remove_dir(&path).unwrap();
        fs::rename(retained_profile, &path).unwrap();

        let request = serde_json::json!({"operation": "reset_default_keys_checked", "expected_locale_binding": &expected});
        let accepted = runtime.preferences_command(outer, serde_json::from_value(request).unwrap()).unwrap();
        let mut source_default = customized.clone();
        // Literal source scan codes 48,50,4b,4d,1c,1d,38,36,36,38.
        source_default.keys = [InputKey::UpArrow, InputKey::DownArrow, InputKey::LeftArrow, InputKey::RightArrow,
            InputKey::Return, InputKey::LeftControl, InputKey::LeftAlt, InputKey::RightShift, InputKey::RightShift, InputKey::LeftAlt];
        source_default.sound_volume = 127;
        source_default.music_volume = 127;
        source_default.show_help = true;
        source_default.brightness = [0, 3, 0];
        assert_eq!(accepted.preferences, source_default);
        assert_eq!(accepted.locale_binding.as_ref(), Some(&expected));
        assert_eq!(runtime.identity().display_settings, before.display_settings);
        assert_eq!(runtime.selected_locale_binding().unwrap().records[48], locale.records[48]);
        let reset_bytes = fs::read(&path).unwrap();
        runtime.preferences_command(outer, PreferencesCommand::Cancel).unwrap();
        assert_eq!(runtime.identity().player_preferences.preferences, source_default, "Cancel after immediate Default does not undo it");
        assert_eq!(fs::read(&path).unwrap(), reset_bytes);
        let mut cold = PreferenceStore::default();
        cold.command(PreferencesCommand::Install { profile_dir: profile.to_string_lossy().into_owned() }).unwrap();
        cold.command(PreferencesCommand::InstallLanguagesChecked {
            cache_dir: isolated.to_string_lossy().into_owned(), expected_locale_binding: expected.clone(),
        }).unwrap();
        assert_eq!(cold.snapshot().preferences, source_default);
        assert_eq!(cold.snapshot().locale_binding, Some(expected.clone()));
        assert_eq!(fs::read(&path).unwrap(), reset_bytes);

        // The same operation respects the existing menu-mode boundary, and
        // changes no gameplay/GAME carrier when admitted while paused.
        runtime.preferences_command(outer, PreferencesCommand::ApplyChecked {
            preferences: customized.clone(), expected_locale_binding: expected.clone(),
        }).unwrap();
        let package = std::env::var_os("MC_CAMPAIGN_LEVEL1_PACKAGE").map_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../generated/mc-bhq8-level1-collision-v5-night")
        }, PathBuf::from);
        let definition = Arc::new(load_mc2_campaign(&package, &default_catalog().unwrap()).unwrap());
        let loading = runtime.new_game(outer, definition).unwrap();
        assert!(runtime.preferences_command(loading.guard(), PreferencesCommand::ResetDefaultKeysChecked {
            expected_locale_binding: expected.clone(),
        }).is_err());
        assert_eq!(runtime.identity(), loading);
        assert_eq!(fs::read(&path).unwrap(), before_bytes);
        runtime.open_level(loading.guard(), &Mc2CampaignAdapter, Mc2CampaignInputs::new(&package), 8870).unwrap();
        let playing = runtime.identity();
        assert!(runtime.preferences_command(playing.guard(), PreferencesCommand::ResetDefaultKeysChecked {
            expected_locale_binding: expected.clone(),
        }).is_err());
        assert_eq!(runtime.identity(), playing);
        assert_eq!(fs::read(&path).unwrap(), before_bytes);
        let nested = runtime.session().unwrap().identity().guard();
        runtime.toggle_icons_and_map(playing.guard(), nested).unwrap();
        runtime.toggle_player_names(playing.guard(), nested).unwrap();
        for action in [super::super::SourceGraphicsAction::Reflections,super::super::SourceGraphicsAction::Shadows,
            super::super::SourceGraphicsAction::Sky,super::super::SourceGraphicsAction::DynamicLighting] {
            runtime.source_graphics(playing.guard(),nested,action).unwrap();
        }
        let paused = runtime.pause(playing.guard()).unwrap();
        let snapshot = runtime.session().unwrap().snapshot();
        let operations = runtime.session().unwrap().backend().replay_operation_count();
        runtime.preferences_command(paused.guard(), PreferencesCommand::ResetDefaultKeysChecked {
            expected_locale_binding: expected,
        }).unwrap();
        assert_eq!(runtime.identity().player_preferences.preferences, source_default);
        assert_eq!(runtime.identity().display_settings, paused.display_settings, "source Default does not set minimap/topbar");
        assert!(!runtime.identity().display_settings.minimap);
        assert!(!runtime.identity().source_player_names_visible, "Default Keys retains auxiliary player-name visibility");
        assert_eq!(runtime.identity().generation, paused.generation);
        assert_eq!(runtime.session().unwrap().snapshot(), snapshot);
        assert_eq!(runtime.session().unwrap().backend().replay_operation_count(), operations);
        assert_eq!(fs::read(&path).unwrap(), reset_bytes);
        fs::remove_dir_all(profile).unwrap();
    }

    #[test]
    fn legacy_profile_graphics_migrate_without_becoming_game_authority() {
        let profile=std::env::temp_dir().join(format!("mc-legacy-graphics-{}-{}",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&profile).unwrap(); let path=profile.join("player-preferences.json");
        let mut expected=PlayerPreferences::default(); expected.player_name="OLIVER".into();
        expected.keys[0]=InputKey::W; expected.sound_volume=61; expected.show_help=false;
        let mut old=serde_json::to_value(&expected).unwrap();
        for field in ["reflections","sky","shadows","dynamic_lighting"] {old[field]=false.into();}
        let bytes=serde_json::to_vec(&old).unwrap(); fs::write(&path,&bytes).unwrap();
        let install=||PreferencesCommand::Install {profile_dir:profile.to_string_lossy().into_owned()};
        let mut store=PreferenceStore::default(); assert_eq!(store.command(install()).unwrap().preferences,expected);
        assert_eq!(fs::read(&path).unwrap(),bytes,"legacy migration never rewrites on read");
        assert!(serde_json::from_value::<PlayerPreferences>(old.clone()).is_err(),"live Apply rejects obsolete GAME fields");
        let graphics=crate::game_runtime::DisplaySettings::default();
        assert!(graphics.reflections&&graphics.shadows&&graphics.sky&&graphics.dynamic_lighting);
        store.command(PreferencesCommand::Apply {preferences:expected.clone()}).unwrap();
        let current:serde_json::Value=serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        for field in ["reflections","sky","shadows","dynamic_lighting"] {assert!(current.get(field).is_none());}
        assert_eq!(serde_json::from_value::<PlayerPreferences>(current).unwrap(),expected);
        old["sky"]="not a boolean".into(); let bad=serde_json::to_vec(&old).unwrap();fs::write(&path,&bad).unwrap();
        let mut invalid=PreferenceStore::default();assert!(invalid.command(install()).unwrap().load_status.starts_with("defaults:"));
        assert_eq!(fs::read(&path).unwrap(),bad);
        let duplicated=String::from_utf8(bytes).unwrap().replacen('{',"{\"player_name\":\"SECOND\",",1);
        fs::write(&path,duplicated.as_bytes()).unwrap();let mut duplicate=PreferenceStore::default();
        assert!(duplicate.command(install()).unwrap().load_status.contains("duplicate preferences field"));
        assert_eq!(fs::read(&path).unwrap(),duplicated.as_bytes());
        fs::remove_dir_all(profile).unwrap();
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
        let expected = before.locale_binding.clone().unwrap();
        store
            .command(PreferencesCommand::InstallLanguagesChecked {
                cache_dir: isolated.to_string_lossy().into_owned(),
                expected_locale_binding: expected.clone(),
            })
            .unwrap();
        store
            .command(PreferencesCommand::ApplyChecked {
                preferences: before.preferences.clone(),
                expected_locale_binding: expected.clone(),
            })
            .unwrap();
        assert_eq!(store.snapshot(), before);
        assert_eq!(
            fs::read(profile.join("player-preferences.json")).unwrap(),
            saved
        );

        // Reorder the source provenance rows in an isolated copy and renew
        // its canonical manifest receipt. Both versions validate against the
        // same real owner bytes, but their complete binding hashes differ.
        // This models a valid cache replacement between managed preflight and
        // native reload without inventing another locale or corrupting data.
        use sha2::{Digest, Sha256};
        let manifest_path = isolated.join("manifest.json");
        let original_manifest = fs::read(&manifest_path).unwrap();
        let mut changed: serde_json::Value = serde_json::from_slice(&original_manifest).unwrap();
        changed["sources"].as_array_mut().unwrap().reverse();
        changed
            .as_object_mut()
            .unwrap()
            .remove("binding_sha256")
            .unwrap();
        let changed_digest = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&changed).unwrap())
        );
        assert_ne!(changed_digest, expected.binding_sha256);
        changed["binding_sha256"] = serde_json::Value::String(changed_digest);
        fs::write(&manifest_path, serde_json::to_vec(&changed).unwrap()).unwrap();
        let replacement = InstalledLanguages::load(&isolated)
            .unwrap()
            .binding(None)
            .unwrap();
        assert_eq!(replacement.identity.id, expected.id);
        assert_ne!(replacement.identity.binding_sha256, expected.binding_sha256);
        let install_request = serde_json::json!({
            "operation": "install_languages_checked",
            "cache_dir": isolated.to_string_lossy(),
            "expected_locale_binding": &expected,
        });
        let apply_request = serde_json::json!({
            "operation": "apply_checked",
            "preferences": &before.preferences,
            "expected_locale_binding": &expected,
        });
        assert!(
            store
                .command(serde_json::from_value(install_request).unwrap())
                .is_err()
        );
        assert!(
            store
                .command(serde_json::from_value(apply_request).unwrap())
                .is_err()
        );
        assert_eq!(
            store.snapshot(),
            before,
            "a changed valid binding cannot replace the retained owner"
        );
        assert_eq!(
            fs::read(profile.join("player-preferences.json")).unwrap(),
            saved,
            "a changed valid binding cannot rewrite the profile"
        );
        fs::write(&manifest_path, original_manifest).unwrap();

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
            let displaced = AssistanceInput { roll: 17, pitch: -9, ..AssistanceInput::default() };
            assert!(!assist.observe(displaced, true, policy));
            for _ in 0..48 { assert!(!assist.observe(displaced, true, policy)); }
            for _ in 0..100 {
                assert!(!assist.observe(AssistanceInput { action: true, ..idle }, true, excluded));
            }
            assert_eq!((assist.previous, assist.unchanged), (sim_session::SteeringObservation::from_axes(17, -9), 48),
                "excluded source branches do not sample activity or clear the idle run");
            assert!(assist.observe(displaced, true, policy));
            assert_eq!(assist.previous, sim_session::SteeringObservation::from_axes(17, -9), "queue39 retains the pre-center sample");
            assert!(!assist.observe(idle, true, policy), "the later actual center starts a new run");
        }
        for _ in 0..48 { assert!(!assist.observe(idle, true, policy)); }
        for _ in 0..100 {
            assert!(!assist.observe(AssistanceInput { roll: 100, action: true, ..idle }, false, policy));
        }
        assert_eq!((assist.previous, assist.unchanged), (sim_session::SteeringObservation::default(), 48));
        assert!(assist.observe(idle, true, policy));
    }

    #[test]
    fn exact_steering_observation_preserves_activity_and_suppressed_samples() {
        use sim_session::{GameCommand, SteeringObservation};
        let zero = AssistanceInput::default();
        let sample = SteeringObservation::try_from([0.45_f32.to_bits(), 0]).unwrap();
        let precise = AssistanceInput { steering_observation:Some(sample), ..zero };
        let policy = AssistancePolicy { playing:true, ..AssistancePolicy::default() };
        let mut assist = FlightAssistance::default();
        assert!(!assist.observe(precise,true,policy));
        for index in 0..100 {
            assert!(!assist.observe(if index % 2 == 0 { zero } else { precise }, true, policy));
            assert_eq!(assist.unchanged, 0, "pre-quantization activity never becomes rounded-axis idle");
        }
        for _ in 0..48 { assert!(!assist.observe(precise, true, policy)); }
        assert!(assist.observe(precise, true, policy), "49 unchanged precise samples still recenter");
        assert_eq!(assist.previous, sample, "recenter is deferred, not an invented zero sample");
        for (enabled, excluded) in [(false,policy),(true,AssistancePolicy { menu_open:true,..policy }),
            (true,AssistancePolicy { network:true,..policy }),(true,AssistancePolicy::default())] {
            for _ in 0..48 { assert!(!assist.observe(precise, true, policy)); }
            for _ in 0..60 { assert!(!assist.observe(zero, enabled, excluded)); }
            assert_eq!((assist.previous,assist.unchanged),(sample,48));
            assert!(assist.observe(precise,true,policy));
        }
        assert!(!assist.observe(zero,true,policy), "the actual centered sample resets the run");
        let legacy = GameCommand::default();
        let bytes = serde_json::to_vec(&legacy).unwrap();
        assert!(!String::from_utf8(bytes.clone()).unwrap().contains("steering_observation"));
        assert_eq!(serde_json::from_slice::<GameCommand>(&bytes).unwrap(), legacy);
        let command: GameCommand = serde_json::from_value(serde_json::json!({
            "steering_observation":[0.45_f32.to_bits(),(-0.0_f32).to_bits()]})).unwrap();
        assert_eq!(command.steering_observation,Some(sample));
        assert_eq!(serde_json::to_value(command).unwrap()["steering_observation"],
            serde_json::json!([0.45_f32.to_bits(),0]));
        for bits in [serde_json::json!([f32::NAN.to_bits(),0]),serde_json::json!([f32::INFINITY.to_bits(),0]),
            serde_json::json!([(-128.00002_f32).to_bits(),0]),serde_json::json!([0,127.00001_f32.to_bits()]),
            serde_json::json!([0]),serde_json::json!([0,0,0]),serde_json::json!([-1,0])] {
            assert!(serde_json::from_value::<GameCommand>(serde_json::json!({"steering_observation":bits})).is_err());
        }
    }

    #[test]
    fn source_help_mode_toggle_is_atomic_and_preserves_unrelated_preferences() {
        let profile = std::env::temp_dir().join(format!("mc-help-mode-{}-{}",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&profile).unwrap();
        let install = || PreferencesCommand::Install { profile_dir:profile.to_string_lossy().into_owned() };
        let mut store = PreferenceStore::default();
        let uninstalled = store.snapshot();
        assert!(store.source_help_mode_toggle().is_err());
        assert_eq!(store.snapshot(),uninstalled);
        store.command(install()).unwrap();
        let request = serde_json::json!({"operation":"source_help_mode","session_guard":
            {"session_id":"1","generation":"2","expected_tick":"3"}});
        assert!(matches!(serde_json::from_value::<PreferencesCommand>(request.clone()).unwrap(),
            PreferencesCommand::SourceHelpMode { .. }));
        assert!(store.command(serde_json::from_value(request.clone()).unwrap()).is_err());
        let mut invented_capability = request;
        invented_capability["help_available"] = false.into();
        assert!(serde_json::from_value::<PreferencesCommand>(invented_capability).is_err(),
            "the pinned source does not expose a host-help availability branch");
        let before = store.current.clone();
        assert_eq!(store.source_help_mode_toggle().unwrap(),Some(false));
        let mut expected = before.clone(); expected.show_help=false;
        assert_eq!(store.current,expected);
        let bytes = fs::read(profile.join("player-preferences.json")).unwrap();
        let mut cold = PreferenceStore::default();
        assert_eq!(cold.command(install()).unwrap().preferences,expected);
        store.command(PreferencesCommand::PreviewAudio { sound_enabled:false,music_enabled:false,
            speech_enabled:false,sound_volume:0,music_volume:0 }).unwrap();
        let preview=store.snapshot();
        assert_eq!(store.source_help_mode_toggle().unwrap(),None);
        assert_eq!(store.snapshot(),preview);
        assert_eq!(fs::read(profile.join("player-preferences.json")).unwrap(),bytes);
        store.command(PreferencesCommand::Cancel).unwrap();
        let destination=store.path.clone().unwrap(); store.path=Some(profile.clone());
        let retained=store.snapshot();
        assert!(store.source_help_mode_toggle().is_err());
        assert_eq!(store.snapshot(),retained);
        assert_eq!(fs::read(&destination).unwrap(),bytes);
        store.path=Some(destination);
        assert_eq!(store.source_help_mode_toggle().unwrap(),Some(true));
        assert_eq!(store.current,before);
        assert!(serde_json::to_value(store.snapshot()).unwrap().get("source_help_mode_admitted").is_none());
        fs::remove_dir_all(profile).unwrap();
    }

    #[test]
    fn source_flight_assistance_preserves_idle_state_and_atomic_profile() {
        let profile = std::env::temp_dir().join(format!("mc-flight-assistance-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&profile).unwrap();
        let mut store = PreferenceStore::default();
        let before = store.snapshot();
        assert!(store.source_flight_assistance_toggle().is_err());
        assert_eq!(store.snapshot(), before);
        let install = || PreferencesCommand::Install { profile_dir: profile.to_string_lossy().into_owned() };
        store.command(install()).unwrap();
        let request = serde_json::json!({"operation":"source_flight_assistance", "session_guard":
            {"session_id":"1","generation":"2","expected_tick":"3"}});
        assert!(matches!(serde_json::from_value::<PreferencesCommand>(request.clone()).unwrap(),
            PreferencesCommand::SourceFlightAssistance { .. }));
        assert!(store.command(serde_json::from_value(request.clone()).unwrap()).is_err());
        let mut invalid = request;
        invalid["session_guard"]["expected_tick"] = "03".into();
        assert!(serde_json::from_value::<PreferencesCommand>(invalid).is_err());
        let original = store.current.clone();
        assert_eq!(store.source_flight_assistance_toggle().unwrap(), Some(true));
        let policy = AssistancePolicy { playing: true, ..AssistancePolicy::default() };
        let input = AssistanceInput { roll: 17, pitch: -9, ..AssistanceInput::default() };
        assert!(!store.observe_input(input, policy));
        for _ in 0..48 { assert!(!store.observe_input(input, policy)); }
        assert_eq!(store.source_flight_assistance_toggle().unwrap(), Some(false));
        for _ in 0..20 { assert!(!store.observe_input(AssistanceInput::default(), policy)); }
        assert_eq!((store.assistance.previous, store.assistance.unchanged), (sim_session::SteeringObservation::from_axes(17, -9), 48));
        assert_eq!(store.source_flight_assistance_toggle().unwrap(), Some(true));
        assert!(store.observe_input(input, policy), "reenabling resumes the real idle carrier");
        assert_eq!(store.assistance_sequence, 1);
        assert_eq!(store.assistance.previous, sim_session::SteeringObservation::from_axes(17, -9));
        assert!(!store.observe_input(AssistanceInput::default(), policy));
        assert_eq!(store.assistance.unchanged, 0);
        let before = store.snapshot();
        let bytes = fs::read(profile.join("player-preferences.json")).unwrap();
        store.command(PreferencesCommand::PreviewAudio { sound_enabled:false, music_enabled:false,
            speech_enabled:false,sound_volume:0,music_volume:0 }).unwrap();
        let preview = store.snapshot();
        assert_eq!(store.source_flight_assistance_toggle().unwrap(), None);
        assert_eq!(store.snapshot(), preview);
        assert_eq!(fs::read(profile.join("player-preferences.json")).unwrap(), bytes);
        store.command(PreferencesCommand::Cancel).unwrap();
        assert_eq!(store.snapshot(), before);
        let mut cold = PreferenceStore::default();
        assert_eq!(cold.command(install()).unwrap().preferences, store.snapshot().preferences);
        assert_eq!(cold.assistance.unchanged, 0, "profile saves the flag, not an idle timer");
        let mut expected = original;
        expected.flight_assistance = true;
        assert_eq!(cold.current, expected, "source action preserves every unrelated preference");
        let saved_path = store.path.clone().unwrap();
        let blocked_path = profile.join("blocked-destination");
        fs::create_dir(&blocked_path).unwrap();
        store.path = Some(blocked_path.clone());
        let before = store.snapshot();
        // Force more failures than the number of candidate names. Every
        // failure must discard its staging file and preserve the live/profile state.
        for _ in 0..129 {
            assert!(store.source_flight_assistance_toggle().is_err());
        }
        assert_eq!(store.snapshot(), before);
        assert_eq!(fs::read(&saved_path).unwrap(), bytes);
        assert_eq!(fs::read_dir(&profile).unwrap().count(), 2);

        fs::remove_dir(blocked_path).unwrap();
        store.path = Some(saved_path);
        assert_eq!(store.source_flight_assistance_toggle().unwrap(), Some(false));
        let mut restarted = PreferenceStore::default();
        assert_eq!(restarted.command(install()).unwrap().preferences, store.snapshot().preferences);
        fs::remove_dir_all(profile).unwrap();
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
                    holder_actor: None,
                }),
                ..GameCommand::default()
            },
            GameCommand {
                mark_spell: Some(SpellMark {
                    spell_id: id,
                    hand: SpellMarkHand::Left,
                    holder_actor: None,
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
    fn legacy_objective_key_is_repaired_without_losing_other_preferences() {
        let time_tag = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "mc-objective-key-test-{}-{time_tag}",
            std::process::id()
        ));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("player-preferences.json");
        let install = || PreferencesCommand::Install {
            profile_dir: dir.to_string_lossy().into_owned(),
        };
        for slot in 0..10 {
            let mut old = PlayerPreferences::default();
            old.player_name = "OLIVER".into();
            old.sound_volume = 23;
            old.keys[slot] = InputKey::O;
            // Occupy the slot's default elsewhere to exercise fallback selection.
            if slot == 0 {
                old.keys[4] = InputKey::UpArrow;
            }
            assert!(old.validate_with_legacy_objective_key(true).is_ok());
            assert!(old.validate().is_err());
            let bytes = serde_json::to_vec(&old).unwrap();
            fs::write(&path, &bytes).unwrap();
            let mut store = PreferenceStore::default();
            let loaded = store.command(install()).unwrap();
            assert_eq!(loaded.load_status, "loaded");
            assert_eq!(loaded.preferences.player_name, old.player_name);
            assert_eq!(loaded.preferences.sound_volume, old.sound_volume);
            assert_ne!(loaded.preferences.keys[slot], InputKey::O);
            for other in 0..10 {
                if other != slot {
                    assert_eq!(loaded.preferences.keys[other], old.keys[other]);
                }
            }
            assert!(loaded.preferences.validate().is_ok());
            assert_eq!(fs::read(&path).unwrap(), bytes, "load must not overwrite the old profile");

            let before = store.snapshot();
            assert!(store.command(PreferencesCommand::Apply { preferences: old }).is_err());
            assert_eq!(store.snapshot(), before);
            assert_eq!(fs::read(&path).unwrap(), bytes, "rejected Apply is atomic");
        }
        let mut old = PlayerPreferences::default();
        old.keys[4] = InputKey::O;
        old.keys[5] = InputKey::O;
        fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
        let mut store = PreferenceStore::default();
        let loaded = store.command(install()).unwrap();
        assert!(loaded.preferences.validate().is_ok());
        assert_ne!(loaded.preferences.keys[4], loaded.preferences.keys[5]);
        for slot in [0, 1, 2, 3, 6, 7, 8, 9] {
            assert_eq!(loaded.preferences.keys[slot], old.keys[slot]);
        }
        old.sound_volume = 128;
        let bytes = serde_json::to_vec(&old).unwrap();
        fs::write(&path, &bytes).unwrap();
        let mut invalid = PreferenceStore::default();
        assert!(invalid.command(install()).unwrap().load_status.starts_with("defaults:"));
        assert_eq!(fs::read(&path).unwrap(), bytes);
        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
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
        prefs.keys = [
            InputKey::W,
            InputKey::Minus,
            InputKey::Equals,
            InputKey::Semicolon,
            InputKey::Backslash,
            InputKey::Comma,
            InputKey::Period,
            InputKey::Slash,
            InputKey::RightShift,
            InputKey::LeftAlt,
        ];
        prefs.sound_volume = 63;
        prefs.show_help = false;
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
        assert_eq!(
            cold.snapshot().preferences.keys,
            store.snapshot().preferences.keys,
            "source punctuation bindings survive a cold profile load"
        );
        assert!(serde_json::to_value(cold.snapshot()).unwrap()["preferences"].get("dynamic_lighting").is_none());
        let audio_command = serde_json::json!({
            "operation": "source_audio",
            "session_guard": { "session_id": "1", "generation": "2", "expected_tick": "3" },
            "action": "speech", "speech_available": true
        });
        assert!(matches!(serde_json::from_value::<PreferencesCommand>(audio_command.clone()).unwrap(),
            PreferencesCommand::SourceAudio { action: SourceAudioAction::Speech,
                speech_available: Some(true), .. }));
        assert!(store.command(serde_json::from_value(audio_command.clone()).unwrap()).is_err());
        let mut invalid_guard = audio_command;
        invalid_guard["session_guard"]["expected_tick"] = serde_json::json!("03");
        assert!(serde_json::from_value::<PreferencesCommand>(invalid_guard).is_err());
        assert!(serde_json::to_value(store.snapshot()).unwrap().get("source_audio_admitted").is_none());
        // The source admission guard and an active dialog preview cannot
        // mutate either the retained preference or its cold profile.
        let before = store.snapshot();
        assert_eq!(store.source_audio_toggle(SourceAudioAction::Speech, false).unwrap(), None);
        assert_eq!(store.snapshot(), before);
        assert_eq!(fs::read(&path).unwrap(), saved);
        store.command(PreferencesCommand::PreviewAudio {
            sound_enabled: false, music_enabled: true, speech_enabled: false,
            sound_volume: 10, music_volume: 20,
        }).unwrap();
        let preview = store.snapshot();
        assert_eq!(store.source_audio_toggle(SourceAudioAction::Sound, true).unwrap(), None);
        assert_eq!(store.snapshot(), preview);
        assert_eq!(fs::read(&path).unwrap(), saved);
        store.command(PreferencesCommand::Cancel).unwrap();
        assert_eq!(store.snapshot(), before);

        for action in [SourceAudioAction::Sound, SourceAudioAction::Music, SourceAudioAction::Speech] {
            assert_eq!(store.source_audio_toggle(action, true).unwrap(), Some(false));
            assert_eq!(store.source_audio_toggle(action, true).unwrap(), Some(true));
        }
        assert_eq!(store.source_audio_toggle(SourceAudioAction::Speech, true).unwrap(), Some(false));
        let mut audio_cold = PreferenceStore::default();
        assert_eq!(audio_cold.command(install()).unwrap().preferences, store.snapshot().preferences);
        assert!(!audio_cold.snapshot().preferences.speech_enabled);
        assert!(audio_cold.snapshot().preferences.sound_enabled);
        assert_eq!(audio_cold.snapshot().preferences.keys, before.preferences.keys);
        assert_eq!(audio_cold.snapshot().preferences.sound_volume, 63);
        let mut uninstalled = PreferenceStore::default();
        let uninstalled_before = uninstalled.snapshot();
        assert!(uninstalled.source_audio_toggle(SourceAudioAction::Sound, true).is_err());
        assert_eq!(uninstalled.snapshot(), uninstalled_before);
        assert_eq!(fs::read(&save).unwrap(), b"existing campaign save");
        fs::remove_file(path).unwrap();
        fs::remove_file(save).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
