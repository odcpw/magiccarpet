//! Instance-owned frontend media clock and ordered source event consumer.
//! Decoding and audio devices are adapters; neither chooses the next segment.
use serde::{Deserialize, Serialize};

pub const MEDIA_CONTRACT: &str = "mc2-frontend-media/v1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct MediaCatalog {
    pub schema: String,
    pub clips: Vec<MediaClip>,
    pub audio: Vec<MediaAudio>,
    pub subtitles: Vec<String>,
    pub subtitles_enabled: bool,
    #[serde(default)]
    pub language: Option<u8>,
    #[serde(default)]
    pub credits: Vec<String>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct MediaClip {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub frame_count: u32,
    pub frames_path: String,
    pub events: Vec<MediaEvent>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct MediaAudio {
    pub kind: String,
    pub bank: u32,
    pub id: i32,
    pub path: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct MediaEvent {
    pub frame: u32,
    pub op: char,
    pub value: i32,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum MediaCommand {
    Install {
        catalog: MediaCatalog,
        #[serde(default = "default_startup")]
        startup: bool,
        #[serde(default = "default_audio_available")]
        audio_available: bool,
    },
    Frame {
        elapsed_us: u32,
        input: bool,
        escape: bool,
    },
    Failed {
        message: String,
    },
}
#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
pub struct MediaAction {
    pub op: char,
    pub value: i32,
    pub asset_path: Option<String>,
    pub volume_units: Option<i32>,
}
#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
pub struct MediaSnapshot {
    pub contract: &'static str,
    pub epoch: u32,
    pub active: bool,
    pub clip_id: String,
    pub frame_index: i32,
    pub fade_in_level: u32,
    pub fade_out_level: u32,
    pub frames_path: String,
    pub width: u32,
    pub height: u32,
    pub subtitle: Option<String>,
    pub subtitle_viewport_offset: u32,
    pub actions: Vec<MediaAction>,
    pub error: Option<String>,
    pub clear_input: bool,
    pub retain_previous_frame: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub campaign_return: Option<super::ending::CampaignReturnSnapshot>,
}
impl Default for MediaSnapshot {
    fn default() -> Self {
        Self {
            contract: MEDIA_CONTRACT,
            epoch: 0,
            active: false,
            clip_id: String::new(),
            frame_index: -1,
            fade_in_level: 32,
            fade_out_level: 0,
            frames_path: String::new(),
            width: 320,
            height: 200,
            subtitle: None,
            subtitle_viewport_offset: 0,
            actions: Vec::new(),
            error: None,
            clear_input: false,
            retain_previous_frame: false,
            campaign_return: None,
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct FrontendMedia {
    ending: super::ending::CampaignReturn,
    audio_available: bool,
    catalog: Option<MediaCatalog>,
    view: MediaSnapshot,
    clock: u64,
    idle: u64,
    deadline: u64,
    ticks: u64,
    pacing: u32,
    next_frame: u32,
    prepared: Option<u32>,
    stop_after_frame: bool,
    event_cursor: usize,
    startup: bool,
    next_attract: bool,
    awaiting_release: bool,
    next_clip: Option<String>,
    subtitles_on: bool,
    retained_subtitle: Option<String>,
    sound_bank: u32,
    music_bank: u32,
    fading: bool,
    fade_elapsed: u64,
    sample_fades: Vec<(i32, i32, i32)>,
    sample_timer_hz: u64,
    sample_timer_fraction: u64,
    title_entry_pending: bool,
}
fn safe_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        && path.split('/').all(|p| !matches!(p, "" | "." | ".."))
}

fn default_startup() -> bool {
    true
}
fn default_audio_available() -> bool { true }

fn source_deadline_us(ticks: u64) -> u64 {
    // Animation.cpp: static_cast<long long>(AnimCurrentTick * AnimTickTime),
    // with AnimTickTime = 8.4f. Rational arithmetic changes real deadlines.
    ((ticks as f32 * 8.4_f32) as u64) * 1_000
}
impl MediaCatalog {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != MEDIA_CONTRACT
            || self.clips.len() > 32
            || self.audio.len() > 512
            || self.subtitles.len() > 4096
        {
            return Err("invalid or oversized frontend media catalog".into());
        }
        if !self.credits.is_empty() && (self.credits.len() != 117
            || self.credits.iter().any(|s| !s.is_ascii() || s.len() > 33)
            || self.credits.split(|s| s == "!").any(|page| page.len() > 8)) {
            return Err("invalid source credits records".into());
        }
        for id in ["welcome", "intro1", "intro2"] {
            if self.clips.iter().filter(|c| c.id == id).count() != 1 {
                return Err(format!("media catalog requires exactly one {id} binding"));
            }
        }
        for clip in &self.clips {
            if !(1..=20000).contains(&clip.frame_count)
                || clip.width == 0
                || clip.width > 640
                || clip.height == 0
                || clip.height > 480
                || !safe_path(&clip.frames_path)
                || clip.events.len() > 4096
            {
                return Err(format!(
                    "invalid media clip {} dimensions/extent/path",
                    clip.id
                ));
            }
            let mut last = 0;
            for event in &clip.events {
                if event.frame < last
                    || event.frame > u32::from(u16::MAX)
                    || event.value < 0
                    || (event.op == 'A' && !(1..=10000).contains(&event.value))
                {
                    return Err(format!(
                        "invalid ordered event in {} at frame {}",
                        clip.id, event.frame
                    ));
                }
                if matches!(event.op, 'Q' | 'U') && event.value as usize >= self.subtitles.len() {
                    return Err(format!("subtitle {} is absent", event.value));
                }
                last = event.frame;
            }
        }
        if self.audio.iter().any(|a| !safe_path(&a.path))
            || self.subtitles.iter().any(|s| s.len() > 16384)
        {
            return Err("invalid media audio path or subtitle extent".into());
        }
        Ok(())
    }
}
impl FrontendMedia {
    pub fn snapshot(&self) -> Option<MediaSnapshot> {
        (self.catalog.is_some() || self.view.error.is_some() || self.ending.active()).then(|| {
            let mut view = self.view.clone();
            let records = self.catalog.as_ref().map_or(&[][..], |c| c.credits.as_slice());
            view.campaign_return = self.ending.snapshot(records);
            if let Some((fade_in, fade_out)) = self.ending.fade_levels() {
                view.fade_in_level = fade_in;
                view.fade_out_level = fade_out;
            }
            view
        })
    }
    pub fn active(&self) -> bool {
        self.view.active || self.awaiting_release || self.title_entry_pending || self.ending.active()
    }
    pub(super) fn campaign_return_active(&self) -> bool { self.ending.active() }
    pub(super) fn played_cutscenes(&self) -> u8 { self.ending.played() }
    pub(super) fn restore_played_cutscenes(&mut self, played: u8) -> Result<(), String> { self.ending.restore_played(played) }
    pub(super) fn reset_campaign_return(&mut self) { self.ending.reset(); }
    pub(super) fn queue_campaign_return(&mut self, cutscene: Option<u8>, credits: bool) {
        self.ending.queue(cutscene, credits);
        if self.ending.active() && self.catalog.is_some() { self.start_campaign_return(); }
    }
    fn start_campaign_return(&mut self) {
        if self.ending.needs_start() {
            let id = format!("cut{}", self.ending.cutscene().expect("cutscene phase"));
            if self.catalog.as_ref().is_some_and(|c| c.clips.iter().any(|clip| clip.id == id)) {
                self.startup = false;
                self.begin(&id);
                self.ending.mark_started();
            } else {
                self.view.error = Some(format!("missing campaign media {id}"));
                self.ending.cutscene_done();
            }
        }
        if self.ending.active() && !self.ending.is_cutscene()
            && self.catalog.as_ref().is_none_or(|c| c.credits.is_empty()) {
            // A non-final CUT may have no credits; its exit drain needs none.
            if self.ending.snapshot(&[]).is_some_and(|s| s.phase == "input_release")
                && self.ending_credits_pending() {
                self.view.error = Some("missing campaign credits".into());
                self.ending.unavailable();
            }
        }
    }
    fn ending_credits_pending(&self) -> bool { self.ending.credits_pending() }
    /// Reset source MainMenu entry state; the next scheduled frontend frame
    /// consumes its audio actions, so an intervening identity read cannot lose
    /// the menu-music restoration.
    pub fn return_to_title(&mut self) {
        if self.catalog.is_some() {
            self.title_entry_pending = true;
        }
    }
    pub fn command(&mut self, command: MediaCommand) -> Result<(), String> {
        self.view.actions.clear();
        self.view.clear_input = false;
        match command {
            MediaCommand::Install { catalog, startup, audio_available } => {
                if self.catalog.is_some() {
                    return Err("media catalog already installed".into());
                }
                catalog.validate()?;
                self.catalog = Some(catalog);
                self.audio_available = audio_available;
                self.startup = startup;
                if self.ending.active() {
                    self.startup = false;
                    self.start_campaign_return();
                } else if startup {
                    self.begin("welcome");
                } else {
                    // A save/direct-game launch first binds media when it
                    // returns to MainMenu, after cold-start intros were bypassed.
                    self.finish();
                }
            }
            MediaCommand::Failed { message } => {
                if self.ending.active() {
                    self.view.error = Some(message.chars().take(2048).collect());
                    self.view.active = false;
                    self.prepared = None;
                    self.awaiting_release = false;
                    self.view.clear_input = true;
                    if self.ending.is_cutscene() && self.catalog.as_ref().is_some_and(|c| !c.credits.is_empty()) {
                        self.ending.cutscene_done();
                    } else {
                        self.ending.unavailable();
                    }
                    return Ok(());
                }
                self.fading = false;
                self.startup = false;
                self.prepared = None;
                self.stop_after_frame = false;
                self.title_entry_pending = false;
                self.finish();
                // Failure returns immediately to usable Title. The normal
                // segment input drain must not keep optional failed media in
                // the runtime's activity/admission predicate.
                self.awaiting_release = false;
                // A broken optional media source cannot trap the user in an
                // idle/retry loop. A new runtime can bind corrected resources.
                self.catalog = None;
                self.view.error = Some(message.chars().take(2048).collect());
            }
            MediaCommand::Frame {
                elapsed_us,
                input,
                escape,
            } => {
                if elapsed_us > 5_000_000 {
                    return Err("media frame elapsed_us exceeds 5 seconds".into());
                }
                if self.ending.active() {
                    if self.catalog.is_none() && !self.ending.exit_drain() { return Ok(()); }
                    if self.ending.needs_start() && self.catalog.is_some() { self.start_campaign_return(); }
                    if self.ending.is_cutscene() && self.ending.needs_start() { return Ok(()); }
                    if !self.ending.is_cutscene() {
                        let records = self.catalog.as_ref().map_or(&[][..], |c| c.credits.as_slice());
                        self.ending.frame(elapsed_us, input || escape, records);
                        self.view.clear_input = self.ending.active();
                        return Ok(());
                    }
                }
                if self.title_entry_pending {
                    self.title_entry_pending = false;
                    self.next_attract = false;
                    self.fading = false;
                    self.startup = false;
                    self.prepared = None;
                    self.stop_after_frame = false;
                    self.finish();
                    return Ok(());
                }
                self.step(u64::from(elapsed_us), input, escape)?;
            }
        }
        Ok(())
    }
    fn action(&mut self, op: char, value: i32) {
        let kind = if matches!(op, 'D' | 'L' | 'M' | 'Z') {
            "music"
        } else {
            "sound"
        };
        let bank = if kind == "music" {
            self.music_bank
        } else {
            self.sound_bank
        };
        let asset_path = self
            .catalog
            .as_ref()
            .and_then(|c| {
                c.audio
                    .iter()
                    .find(|a| a.kind == kind && a.bank == bank && a.id == value)
            })
            .map(|a| a.path.clone());
        self.view.actions.push(MediaAction {
            op,
            value,
            asset_path,
            volume_units: None,
        });
    }
    fn tick_samples(&mut self, elapsed: u64) {
        self.sample_timer_fraction += elapsed * self.sample_timer_hz;
        let callbacks = (self.sample_timer_fraction / 1_000_000) as i32;
        self.sample_timer_fraction %= 1_000_000;
        if callbacks == 0 {
            return;
        }
        for (id, current, target) in &mut self.sample_fades {
            if current == target {
                continue;
            }
            *current += (*target - *current).clamp(-callbacks, callbacks);
            self.view.actions.push(MediaAction {
                op: 'G',
                value: *id,
                asset_path: None,
                volume_units: Some(*current),
            });
        }
    }
    fn sample_event(&mut self, op: char, id: i32) {
        match op {
            'H' | 'R' => {
                if !self.sample_fades.iter().any(|s| s.0 == id) {
                    let volume = if op == 'H' { 0 } else { 127 };
                    self.sample_fades.push((id, volume, volume));
                }
            }
            'F' | 'O' | 'P' => {
                if let Some(sample) = self.sample_fades.iter_mut().find(|s| s.0 == id) {
                    sample.2 = match op {
                        'P' => 80,
                        'O' => 127,
                        _ => 0,
                    };
                    if self.sample_timer_hz == 0 {
                        self.sample_timer_hz = if op == 'F' { 120 } else { 60 };
                        self.sample_timer_fraction = 0;
                    }
                }
            }
            'T' | 'E' | 'S' if id == 0 || op == 'E' => {
                self.sample_fades.clear();
                self.sample_timer_hz = 0;
                self.sample_timer_fraction = 0;
            }
            'T' => self.sample_fades.retain(|s| s.0 != id),
            _ => {}
        }
    }
    fn begin(&mut self, id: &str) {
        let retain_previous = self.startup && self.view.clip_id == "intro1" && id == "intro2";
        let clip = self
            .catalog
            .as_ref()
            .and_then(|c| c.clips.iter().find(|c| c.id == id))
            .expect("validated startup media binding");
        self.view.epoch = self.view.epoch.wrapping_add(1);
        self.view.active = true;
        self.view.clip_id = id.into();
        self.view.frames_path = clip.frames_path.clone();
        self.view.width = clip.width;
        self.view.height = clip.height;
        self.view.frame_index = if id == "welcome" { 0 } else { -1 };
        self.view.fade_in_level = if id == "welcome" { 0 } else { 32 };
        self.view.fade_out_level = 0;
        self.fading = false;
        self.view.subtitle = None;
        self.view.subtitle_viewport_offset = 0;
        self.view.clear_input = true;
        self.view.retain_previous_frame = retain_previous;
        self.clock = 0;
        self.deadline = 0;
        self.ticks = 0;
        self.pacing = 3;
        self.next_frame = 0;
        self.prepared = None;
        self.stop_after_frame = false;
        self.event_cursor = 0;
        self.subtitles_on = false;
        self.retained_subtitle = None;
    }
    fn finish(&mut self) {
        self.action('T', 0);
        self.sample_event('T', 0);
        self.action('X', 0);
        self.view.active = false;
        self.view.frame_index = -1;
        self.view.subtitle = None;
        self.view.subtitle_viewport_offset = 0;
        self.view.clear_input = true;
        self.awaiting_release = true;
        self.next_clip = None;
        self.clock = 0;
        self.idle = 0;
        // MainMenu_76FA0: LoadSounds(0), StartMusic(4,127).
        self.sound_bank = 0;
        self.action('E', 0);
        self.action('M', 4);
    }
    fn segment_done(&mut self) {
        if self.startup && self.view.clip_id == "intro1" {
            // Intros_76D10 only clears subtitle state and drains input here.
            // INTRO2 frame0 E and frame1 X own sample/music cleanup.
            self.complete_segment();
            return;
        }
        self.fading = true;
        self.fade_elapsed = 0;
        self.view.fade_out_level = 0;
    }
    fn complete_segment(&mut self) {
        self.fading = false;
        if self.ending.is_cutscene() {
            self.action('T', 0);
            self.sample_event('T', 0);
            self.action('X', 0);
            self.view.active = false;
            self.view.clear_input = true;
            self.ending.cutscene_done();
            self.start_campaign_return();
            return;
        }
        let next = if self.startup {
            match self.view.clip_id.as_str() {
                "welcome" => Some("intro1"),
                "intro1" => Some("intro2"),
                _ => None,
            }
        } else {
            None
        };
        if let Some(next) = next {
            let next = next.to_owned();
            if self.view.clip_id == "welcome" {
                self.view.frame_index = -1;
            }
            self.view.clear_input = true;
            self.awaiting_release = true;
            self.clock = 0;
            self.next_clip = Some(next);
        } else {
            self.startup = false;
            self.finish();
        }
    }
    fn step(&mut self, elapsed: u64, input: bool, escape: bool) -> Result<(), String> {
        self.clock = self.clock.saturating_add(elapsed);
        self.tick_samples(elapsed);
        if self.stop_after_frame {
            self.stop_after_frame = false;
            self.segment_done();
            return Ok(());
        }
        if self.fading {
            self.fade_elapsed += elapsed;
            if self.fade_elapsed >= 160_000 {
                self.complete_segment();
            } else {
                self.view.fade_out_level = (self.fade_elapsed / 10_000) as u32;
            }
            return Ok(());
        }
        if self.awaiting_release {
            self.view.clear_input = true;
            if input || escape {
                // ShowWelcomeScreen/Intros drain held input before their
                // 50ms delay; time spent holding skip cannot consume it.
                self.clock = 0;
            } else if self.clock >= 50_000 {
                self.awaiting_release = false;
                self.view.clear_input = false;
                if let Some(id) = self.next_clip.take() {
                    self.begin(&id);
                }
            }
            return Ok(());
        }
        if !self.view.active {
            if self.catalog.is_none() {
                return Ok(());
            }
            self.idle = if input || escape {
                0
            } else {
                self.idle.saturating_add(elapsed)
            };
            if self.idle >= 61_000_000 {
                let id = if self.next_attract {
                    "intro2"
                } else {
                    "intro1"
                };
                self.next_attract = !self.next_attract;
                self.begin(id);
            }
            return Ok(());
        }
        if self.view.clip_id == "welcome" {
            self.view.fade_in_level = (self.clock / 10_000).min(31) as u32;
            // (hundredths_since_start / 100) <= 20 in ShowWelcomeScreen.
            if input || escape || self.clock >= 21_000_000 {
                self.segment_done();
            }
            return Ok(());
        }
        if escape || (input && !self.ending.is_cutscene()) {
            // PlayInfoFmv runs events and decodes the current frame before
            // sub_75CB0 observes skip. Skip bypasses the wait, but DrawFrame
            // still installs its palette/subtitles and blits. Escape is also
            // checked only after that frame. Keep the observation visible for
            // one client presentation before starting segment cleanup.
            if self.prepared.is_some() || self.prepare_frame() {
                self.present_prepared_frame();
                self.stop_after_frame = true;
            }
            return Ok(());
        }
        if self.prepared.is_some() {
            if self.clock >= self.deadline {
                self.present_prepared_frame();
            } else {
                return Ok(());
            }
        }
        self.prepare_frame();
        Ok(())
    }
    fn present_prepared_frame(&mut self) {
        let frame = self.prepared.take().expect("prepared media frame");
        self.view.frame_index = frame as i32;
        self.view.retain_previous_frame = false;
        self.view.subtitle = self.retained_subtitle.clone();
        self.view.subtitle_viewport_offset = if self.subtitles_on { 0x26C0 } else { 0 };
        self.next_frame += 1;
    }
    fn prepare_frame(&mut self) -> bool {
        let clip = self
            .catalog
            .as_ref()
            .unwrap()
            .clips
            .iter()
            .find(|c| c.id == self.view.clip_id)
            .unwrap();
        if self.next_frame >= clip.frame_count {
            self.segment_done();
            return false;
        }
        let events: Vec<_> = clip.events[self.event_cursor..]
            .iter()
            .take_while(|e| e.frame == self.next_frame)
            .cloned()
            .collect();
        for event in events {
            self.event_cursor += 1;
            self.sample_event(event.op, event.value);
            match event.op {
                'A' => self.pacing = event.value as u32,
                'B' => {
                    self.music_bank = event.value as u32;
                    self.action('X', 0);
                    self.action('B', event.value);
                }
                'E' => {
                    self.sound_bank = event.value as u32;
                    self.action('T', 0);
                    self.action('E', event.value);
                }
                'V' => {
                    let catalog = self.catalog.as_ref().unwrap();
                    self.subtitles_on = if self.ending.is_cutscene() {
                        self.ending.cutscene() != Some(6)
                            && catalog.language.map_or(catalog.subtitles_enabled, |language| language != 2 || !self.audio_available)
                    } else { catalog.subtitles_enabled };
                }
                'Q' | 'U' if self.subtitles_on => {
                    self.retained_subtitle =
                        Some(self.catalog.as_ref().unwrap().subtitles[event.value as usize].clone())
                }
                'K' | 'W' => self.retained_subtitle = None,
                'Y' => {
                    self.subtitles_on = false;
                    self.retained_subtitle = None;
                }
                op => self.action(op, event.value),
            }
        }
        self.ticks += u64::from(self.pacing);
        self.deadline = source_deadline_us(self.ticks);
        self.prepared = Some(self.next_frame);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn media() -> FrontendMedia {
        let clips = ["welcome", "intro1", "intro2"]
            .map(|id| MediaClip {
                id: id.into(),
                width: 320,
                height: 200,
                frame_count: 2,
                frames_path: format!("{id}.frames"),
                events: if id == "intro1" {
                    vec![
                        MediaEvent {
                            frame: 0,
                            op: 'A',
                            value: 10,
                        },
                        MediaEvent {
                            frame: 0,
                            op: 'V',
                            value: 0,
                        },
                        MediaEvent {
                            frame: 0,
                            op: 'U',
                            value: 0,
                        },
                        MediaEvent {
                            frame: 0,
                            op: 'S',
                            value: 3,
                        },
                    ]
                } else {
                    vec![]
                },
            })
            .to_vec();
        let mut owner = FrontendMedia::default();
        owner
            .command(MediaCommand::Install {
                startup: true,
                audio_available: true,
                catalog: MediaCatalog {
                    schema: MEDIA_CONTRACT.into(),
                    clips,
                    audio: vec![],
                    subtitles: vec!["owner localized line".into()],
                    subtitles_enabled: true,
                    language: None,
                    credits: vec![],
                },
            })
            .unwrap();
        owner
    }
    fn frame(owner: &mut FrontendMedia, elapsed_us: u32, input: bool) {
        owner
            .command(MediaCommand::Frame {
                elapsed_us,
                input,
                escape: false,
            })
            .unwrap();
    }
    fn skip(owner: &mut FrontendMedia) {
        frame(owner, 0, true);
        frame(owner, 0, false);
        if owner.fading {
            frame(owner, 160_000, false);
        }
        frame(owner, 50_000, false);
    }
    #[test]
    fn source_float_deadline_retains_truncation_at_real_intro_keyframes() {
        assert_eq!(source_deadline_us(10), 84_000);
        assert_eq!(source_deadline_us(30), 251_000);
        assert_eq!(source_deadline_us(100), 839_000);
    }

    #[test]
    fn missing_campaign_catalog_drains_input_and_preserves_played_history() {
        let mut owner = FrontendMedia::default();
        owner.queue_campaign_return(Some(6), true);
        assert_eq!(owner.played_cutscenes(), 32);
        assert!(owner.snapshot().unwrap().campaign_return.unwrap().blocking);
        owner.command(MediaCommand::Failed { message: "missing media".into() }).unwrap();
        frame(&mut owner, 0, true);
        assert!(owner.campaign_return_active());
        frame(&mut owner, 0, false);
        assert!(!owner.campaign_return_active());
        assert_eq!(owner.played_cutscenes(), 32);
        assert_eq!(owner.snapshot().unwrap().error.as_deref(), Some("missing media"));
    }

    #[test]
    fn source_cut_subtitles_use_language_and_actual_audio_capability() {
        for (cut, audio_available, expected) in [(1, true, false), (1, false, true), (6, false, false)] {
            let mut catalog = media().catalog.unwrap();
            let mut clip = catalog.clips.iter().find(|clip| clip.id == "intro1").unwrap().clone();
            clip.id = format!("cut{cut}");
            catalog.clips.push(clip);
            catalog.language = Some(2);
            catalog.subtitles_enabled = false;
            let mut owner = FrontendMedia::default();
            owner.queue_campaign_return(Some(cut), false);
            owner.command(MediaCommand::Install { catalog, startup: false, audio_available }).unwrap();
            assert!(owner.prepare_frame());
            assert_eq!(owner.subtitles_on, expected);
            assert_eq!(owner.retained_subtitle.is_some(), expected);
        }
    }

    #[test]
    fn first_title_after_direct_game_binds_menu_music_without_cold_start() {
        let mut catalog = media().catalog.unwrap();
        catalog.audio.push(MediaAudio {
            kind: "music".into(),
            bank: 0,
            id: 4,
            path: "audio/menu.wav".into(),
        });
        let mut owner = FrontendMedia::default();
        owner
            .command(MediaCommand::Install {
                catalog,
                startup: false,
                audio_available: true,
            })
            .unwrap();
        assert!(!owner.view.active);
        assert!(owner.view.clip_id.is_empty());
        assert!(owner.view.clear_input);
        assert_eq!(
            owner.view.actions.last().unwrap().asset_path.as_deref(),
            Some("audio/menu.wav")
        );
        frame(&mut owner, 50_000, false);
        assert!(!owner.active());
        for _ in 0..12 {
            frame(&mut owner, 5_000_000, false);
        }
        assert!(!owner.view.active);
        frame(&mut owner, 1_000_000, false);
        assert_eq!(owner.view.clip_id, "intro1");
    }

    #[test]
    fn source_events_precede_deadline_palette_and_retained_subtitle_presentation() {
        let mut owner = media();
        skip(&mut owner);
        assert_eq!(owner.view.clip_id, "intro1");
        frame(&mut owner, 0, false);
        assert_eq!(owner.view.frame_index, -1);
        assert_eq!(owner.view.subtitle, None);
        assert_eq!(owner.view.actions[0].op, 'S');
        frame(&mut owner, 83_999, false);
        assert_eq!(owner.view.frame_index, -1);
        frame(&mut owner, 1, false);
        assert_eq!(owner.view.frame_index, 0);
        assert_eq!(owner.view.subtitle.as_deref(), Some("owner localized line"));
        assert_eq!(owner.view.subtitle_viewport_offset, 0x26C0);
        assert!(owner.view.actions.is_empty());
        frame(&mut owner, 0, false);
        frame(&mut owner, 84_000, false);
        assert_eq!(owner.view.frame_index, 1);
        assert_eq!(owner.view.subtitle.as_deref(), Some("owner localized line"));
    }
    #[test]
    fn skip_prepares_events_and_presents_frame_before_segment_cleanup() {
        let mut owner = media();
        skip(&mut owner);
        frame(&mut owner, 0, true);
        assert_eq!(owner.view.clip_id, "intro1");
        assert_eq!(owner.view.frame_index, 0);
        assert_eq!(owner.view.subtitle.as_deref(), Some("owner localized line"));
        assert_eq!(owner.view.subtitle_viewport_offset, 0x26C0);
        assert_eq!(owner.view.actions[0].op, 'S');
        assert!(owner.stop_after_frame);
        frame(&mut owner, 0, false);
        assert_eq!(owner.view.clip_id, "intro1");
        assert!(owner.awaiting_release);
        assert!(owner.view.actions.is_empty());
        frame(&mut owner, 50_000, false);
        assert_eq!(owner.view.clip_id, "intro2");
        assert_eq!(owner.view.frame_index, -1);
        assert!(owner.view.retain_previous_frame);
    }
    #[test]
    fn escape_interrupts_wait_and_presents_prepared_frame_once() {
        let mut owner = media();
        skip(&mut owner);
        frame(&mut owner, 0, false);
        assert_eq!(owner.view.frame_index, -1);
        owner
            .command(MediaCommand::Frame {
                elapsed_us: 1,
                input: false,
                escape: true,
            })
            .unwrap();
        assert_eq!(owner.view.frame_index, 0);
        assert_eq!(owner.next_frame, 1);
        assert!(owner.view.actions.is_empty());
        assert!(owner.stop_after_frame);
        frame(&mut owner, 1, true);
        assert_eq!(owner.next_frame, 1);
        assert!(owner.awaiting_release);
    }
    #[test]
    fn held_skip_cannot_skip_following_segment_and_idle_alternates() {
        let mut owner = media();
        frame(&mut owner, 0, true);
        frame(&mut owner, 160_000, true);
        frame(&mut owner, 100_000, true);
        assert_eq!(owner.view.clip_id, "welcome");
        frame(&mut owner, 0, false);
        assert_eq!(owner.view.clip_id, "welcome");
        frame(&mut owner, 49_999, false);
        assert_eq!(owner.view.clip_id, "welcome");
        frame(&mut owner, 1, false);
        assert_eq!(owner.view.clip_id, "intro1");
        skip(&mut owner);
        assert_eq!(owner.view.clip_id, "intro2");
        skip(&mut owner);
        assert!(!owner.view.active);
        assert_eq!(owner.view.actions.len(), 0);
        for _ in 0..12 {
            frame(&mut owner, 5_000_000, false);
        }
        assert!(!owner.view.active);
        frame(&mut owner, 1_000_000, false);
        assert_eq!(owner.view.clip_id, "intro1");
        skip(&mut owner);
        for _ in 0..12 {
            frame(&mut owner, 5_000_000, false);
        }
        frame(&mut owner, 1_000_000, false);
        assert_eq!(owner.view.clip_id, "intro2");
    }
    #[test]
    fn title_reentry_restores_music_on_scheduled_frame_and_restarts_idle() {
        let mut owner = media();
        skip(&mut owner);
        skip(&mut owner);
        skip(&mut owner);
        frame(&mut owner, 5_000_000, false);
        owner.next_attract = true;
        owner.return_to_title();
        let _observation = owner.snapshot();
        assert!(owner.active());
        frame(&mut owner, 0, false);
        assert!(owner.view.clear_input);
        assert!(
            owner
                .view
                .actions
                .iter()
                .any(|a| a.op == 'M' && a.value == 4)
        );
        frame(&mut owner, 50_000, false);
        for _ in 0..12 {
            frame(&mut owner, 5_000_000, false);
        }
        assert!(!owner.view.active);
        frame(&mut owner, 1_000_000, false);
        assert_eq!(owner.view.clip_id, "intro1");
    }

    #[test]
    fn malformed_binding_and_device_failure_leave_recoverable_title() {
        let mut owner = media();
        let mut catalog = owner.catalog.clone().unwrap();
        catalog.clips[1].events[0].value = 0;
        assert!(catalog.validate().unwrap_err().contains("ordered event"));
        catalog.clips[1].events[0].value = 10;
        catalog.clips[1].frames_path = "../INTRO.DAT".into();
        assert!(catalog.validate().is_err());
        owner
            .command(MediaCommand::Failed {
                message: "missing /override/intro1.frames".into(),
            })
            .unwrap();
        assert!(!owner.active());
        assert!(owner.view.clear_input);
        frame(&mut owner, 49_999, false);
        assert!(!owner.active());
        frame(&mut owner, 1, false);
        assert!(!owner.active());
        assert_eq!(
            owner.view.error.as_deref(),
            Some("missing /override/intro1.frames")
        );
        for _ in 0..20 {
            frame(&mut owner, 5_000_000, false);
        }
        assert!(!owner.active());
    }
}
