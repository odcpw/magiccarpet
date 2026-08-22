# Browser-first UX, onboarding, accessibility, and comfort contract

> **Subordinate as of 2026-08-21.** Browser-first is retired. Reuse relevant UX,
> accessibility and comfort requirements for Unity or optional browser clients;
> this file is not a foundation or release gate.

Version: 1.0  
Status: optional-client UX research
Owner bead: `mc-9od`  
Applies first to: current stable 64-bit desktop Chrome on supported Windows
and Linux

## Outcome

A friend who receives a room link can understand what is loading, configure
essential access and comfort settings, enter flight deliberately, recover from
browser interruptions, and reach a rematch without needing an account or
reading external instructions.

The first social demo targets:

- semantic browser UI conforming to WCAG 2.2 AA where applicable;
- an in-canvas game UI that applies the same perceivable, operable,
  understandable and robust principles within Unity Web's accessibility
  limits;
- keyboard/mouse as the certified baseline;
- controller as a required S1 proof target, not a silent assumption;
- no required touch, mobile, voice-chat, screen-reader-complete flight or
  native-VR support in S0/S1.

WCAG conformance applies to the authored HTML shell as a web-content target.
The project does not claim that a real-time 3D canvas is automatically WCAG
conformant. Game accessibility additionally follows the Xbox Accessibility
Guidelines as design and test guidance, not as a legal certification.

## Governing principles

1. **Settings before spectacle.** A first-run access panel is available before
   flashing, camera motion, spatial audio or the 3D world begins.
2. **Explicit entry into capture.** Pointer lock, fullscreen and audio are
   requested only after a clearly labelled user action.
3. **No permission is a valid state.** Refusing fullscreen, pointer lock,
   persistent storage or audio never creates an indefinite loader.
4. **Authority never follows presentation.** Comfort, camera, UI, caption and
   local audio settings do not alter authoritative movement, hit, resource,
   world or result rules.
5. **No single sensory channel.** Essential warnings and state changes use at
   least two of text/icon/shape/motion/audio/haptic channels, with color and
   audio never the sole channel.
6. **Recover before reload.** Focus loss, Escape, hidden tabs, controller
   removal and network stalls explain the state and offer an exact next action.
7. **Safe defaults are genuinely safe.** The default avoids motion blur,
   excessive shake, repeated flashes, surprise loudness and essential
   hold/chord actions.
8. **Accessibility is test data.** Settings, input profiles and first-session
   states are versioned fixtures in CI and browser automation.

## First-session journey

```text
Room link
-> semantic landing page
-> capability and build preflight
-> first-run access and comfort panel
-> explicit "Enter flight" gesture
-> audio activation and optional pointer lock
-> lobby/readiness
-> safe orientation bubble
-> learn movement, aim, map and one spell
-> match
-> result
-> rematch/new room
```

### 1. Landing

The HTML document exposes a meaningful title, language, heading, status region
and main landmark. Before Unity downloads, it shows:

- game/demo identity and private-test status;
- room-code destination or create-room action;
- expected compressed download and approximate progress;
- supported browser/OS/input statement;
- privacy/telemetry statement for the prototype;
- `Accessibility and comfort` and `Troubleshooting` controls;
- a cancellable start action.

No canvas, video or music steals focus. Loading progress uses a semantic
progress element or equivalent labelled status and never communicates through
animation alone.

### 2. Preflight

Preflight produces one of `pass`, `degraded`, or `blocked` for:

- browser and WebGL2;
- WebAssembly and memory allocation;
- secure context and transport;
- storage/cache availability;
- keyboard/mouse and detected controller;
- audio activation state;
- build, protocol, rules and content compatibility.

A blocked result names the failed capability and remedy. A degraded result
names the disabled feature and allows an informed continuation when safe.
There is no generic `Something went wrong` terminal state.

### 3. First-run access panel

The first interactive screen offers:

- text/UI scale;
- high-contrast UI;
- caption mode and size;
- master/music/voice/effects/ambience levels;
- reduced camera motion;
- horizon stabilization;
- camera shake;
- flash/lightning intensity;
- mouse/controller sensitivity and X/Y inversion;
- hold versus toggle for continuous actions;
- keyboard/mouse or controller preset;
- `Use safe defaults`;
- `Reset accessibility settings`.

The player may continue without visiting every field. Focus order is stable;
changes do not unexpectedly submit or move to a new screen.

### 4. Entering flight

One explicit `Enter flight` activation may request audio playback and pointer
lock. Fullscreen is separate and optional. The UI explains:

- mouse movement will control view/turning;
- `Escape` releases capture;
- the keyboard command that re-enters capture;
- controller use does not require pointer lock.

If pointer lock is denied or unavailable, the demo provides a bounded
uncaptured mouse mode and keyboard/controller turning. It may warn about
reduced precision but must not loop permission prompts.

### 5. Orientation bubble

The first spawn is protected from immediate combat pressure while the player
learns:

1. accelerate/decelerate;
2. turn/bank and climb/dive;
3. brake/steady flight;
4. recenter view and change comfort preset;
5. aim and cast the direct projectile;
6. identify health, magic/resource and objective;
7. open/close the minimap or tactical map;
8. release and reacquire browser capture.

Prompts advance from performed actions, not from a fixed reading timer.
Every prompt can be replayed, skipped, dismissed and reopened from Help.
Skipping does not change competitive authority.

## Browser shell accessibility target

The semantic shell targets WCAG 2.2 AA, including:

- keyboard operation and no keyboard trap;
- logical, visible and unobscured focus;
- labelled controls, errors and status messages;
- predictable focus/input behavior;
- page and part language metadata;
- text alternatives for meaningful non-text content;
- color-independent meaning;
- minimum 4.5:1 normal-text and 3:1 large-text contrast;
- 3:1 non-text control/state contrast;
- text resize and reflow without loss of shell function;
- captions or equivalent text for prerecorded essential media;
- no content exceeding the three-flashes threshold;
- at least 24 by 24 CSS pixel targets or compliant spacing, with 44 by 44
  preferred for primary controls;
- no drag-only shell function;
- accessible error identification, suggestion and recovery.

Custom canvas controls that duplicate shell functions do not replace the
semantic controls. Lobby, room-code forms, settings, errors, download status
and recovery actions remain ordinary HTML wherever practical.

## Input contract

### Action model

Bindings refer to semantic actions rather than physical keys:

```text
MoveForward / MoveBackward
TurnLeft / TurnRight
StrafeLeft / StrafeRight
Climb / Dive
BrakeOrSteady
PrimaryCast
SecondaryCastOrSelect
NextAbility / PreviousAbility
OpenMap
ObjectiveRecall
RecenterCamera
MenuOrReleaseCapture
PushToTalk (deferred; unbound in S1)
```

All gameplay and menu actions can be rebound for each supported device except
the browser-reserved escape gesture. Conflicts are detected before save.
Defaults are provided for common QWERTY and controller layouts, but bindings
store physical/control paths rather than assuming English characters.

### Certified alternatives

- All shell/menu operations: keyboard alone and pointer alone.
- All gameplay operations: keyboard/mouse and controller.
- Analog mouse/stick turning: digital left/right alternative.
- Continuous press: toggle alternative where it does not create an
  authoritative ambiguity.
- Multi-button chord: single-action alternative.
- Repeated rapid activation: no essential S1 mechanic requires it.
- Dragging sliders: arrow keys, increment buttons or numeric/select
  alternatives.

Sensitivity is adjustable to at least 50% below and above default. X and Y
inversion are independent. Dead zone and response curve are configurable for
controller. Device switching is live; controller disconnection clears the
affected actions and opens a non-authoritative recovery overlay.

Remapping cannot bind one physical activation to multiple conflicting
authoritative commands. Presentation-only conveniences may be many-to-one.

## Motion and camera comfort

The presentation rig exposes four named starting profiles:

| Profile | Roll | Lag/sway | Shake | Speed effects | Horizon |
|---|---:|---:|---:|---:|---:|
| Expressive | design default | design default | low | low | partial |
| Steady | reduced | reduced | off | reduced | high |
| Minimal motion | off | off | off | off | locked |
| Custom | player values | player values | player values | player values | player value |

The first-run default is `Steady` until playtesting establishes that
`Expressive` is broadly comfortable. The player can change profile during
flight without restarting.

Configurable presentation values include:

- field of view within a tested, bounded range;
- camera roll/bank contribution;
- horizon stabilization;
- positional and rotational follow lag;
- impact shake;
- head/camera bob and idle sway;
- speed lines, vignette and chromatic effects;
- motion blur, off by default in the browser proof;
- lightning brightness and flash substitution;
- damage pulse intensity;
- camera recenter mode.

Camera settings cannot reveal hidden authoritative information, change
projectile origin, alter collision or change server input magnitudes. The
crosshair uses the declared aim transform rather than a camera-only illusion.

## Flash, color and visual intensity

- No authored sequence intentionally exceeds three flashes in one second or
  the WCAG general/red flash thresholds.
- Lightning has `full`, `reduced` and `no-flash` presentation modes; reduced
  and no-flash retain thunder, sky tint, icon and caption cues.
- Damage, spell tiers, factions, objectives and hazards combine color with
  shape, icon, pattern, label or spatial treatment.
- A high-contrast mode affects presentation only and is tested under common
  color-vision simulations.
- Essential world interactables remain distinguishable on the minimum visual
  quality tier without bloom, fog detail or particle density.
- Automatically moving instructional text can be paused or disabled.

## Text, captions and audio alternatives

### Text

- UI scale supports at least 100%, 125%, 150%, 175% and 200%.
- Essential canvas text remains inside safe bounds at 200% or moves to the
  semantic overlay.
- Fonts declare Latin coverage for S1 and a fallback strategy before any
  additional locale is promised.
- No essential information is baked into a texture.

### Captions

Caption modes are `off`, `speech`, and `speech + essential sounds`. The
first-run default is `speech + essential sounds`.

Captions support:

- speaker/source identity;
- direction or off-screen indication when tactically essential;
- environmental cues such as thunder, incoming projectile, stronghold alarm
  or creature warning without exposing hidden information;
- adjustable text size, background opacity and duration;
- no reliance on color for speaker identity;
- no display substantially before the corresponding authoritative/presentation
  event.

Music and ambience remain optional layers. Lowering or muting them cannot
remove the only available objective, threat, spell or result cue. Spatial
audio has a visual/caption alternative for essential direction.

## Settings and safe-start schema

Settings are versioned, validated and separated by domain:

```text
SettingsEnvelope
  schemaVersion
  input
  cameraComfort
  visualAccessibility
  captions
  audio
  ui
  locale
  diagnosticsConsent
```

Rules:

- local settings load before the 3D scene;
- corrupt or future-version settings fall back safely and retain a redacted
  diagnostic;
- defaults are portable between browser and native where the capability
  exists;
- bindings refer to semantic action IDs and device control paths;
- accessibility reset does not erase room identity or unrelated progress;
- `safe start` can be invoked from the landing page and a documented query
  parameter;
- settings can be exported/imported as validated JSON later, but no S1 account
  is required;
- telemetry never contains raw key sequences, room tokens or accessibility
  diagnoses.

Safe start applies: semantic shell only, 150% UI, high contrast, captions with
essential sounds, steady camera, shake/blur/speed effects off, no lightning
flash, conservative audio, default keyboard navigation and no automatic
fullscreen.

## Localization readiness

- All player-facing strings use stable message IDs.
- No string concatenation constructs grammar.
- Variables are named and documented.
- Plural, number, date and duration formatting use locale-aware formatters.
- Layout supports at least 30% text expansion.
- A long-string pseudolocale and accented pseudolocale run in CI.
- Mirrored/RTL shell layout is demonstrated before promising an RTL locale.
- Input prompts render semantic action plus current binding, not hard-coded
  English key names.
- Fonts and fallback glyph coverage are build-manifest data.
- Server errors carry stable codes; clients localize them.

## Browser interruption UX

This document owns presentation expectations; `mc-x8i` owns authoritative
state transitions.

| Event | Immediate client behavior | Player-facing recovery |
|---|---|---|
| Focus or pointer lock lost | Clear sampled input; stop local cast repeats | Overlay reason and `Resume flight` |
| Escape/fullscreen exit | Release capture; keep menu keyboard focus | Resume, settings, leave room |
| Audio blocked | Continue muted; show non-modal status | `Enable audio` explicit gesture |
| Tab hidden | Clear input; stop costly presentation work | On return show repair progress |
| Network stalled | Freeze speculative side effects at bound | Connection state and elapsed grace |
| Controller removed | Clear device actions | Reconnect or switch device |
| Cache corrupt | Abandon bad artifact | Verified re-download with progress |
| Build mismatch | Do not join authority | Reload/update explanation |
| Reconnect expired | No silent new identity | Results/spectate/room return as allowed |

No overlay implies that the authoritative match paused. Solo may use an
explicit server-side pause rule later; S1 assumes no pause on focus loss.

## Test protocols

Every protocol records build/content/rules IDs, browser/OS, input device,
settings envelope, timings, result, screenshots/video where appropriate and
the first failed step.

### Automated shell gates

- keyboard traversal, focus order and focus visibility;
- accessible names/roles/status/error associations;
- color and text contrast;
- 200% zoom/reflow and 30% pseudolocalization expansion;
- target sizes and drag alternatives;
- reduced-motion media preference and safe-start query;
- denied audio, storage, fullscreen and pointer-lock paths;
- corrupt/future settings migration;
- room error and reconnect status announcements.

Automated checks assist but do not replace manual assistive-technology and
motion-comfort testing.

### Manual first-session protocols

1. **Unprepared friend:** room link to controlled flight without explanation.
2. **Keyboard only:** landing, settings, join, tutorial, match exit and rematch.
3. **Controller only:** first panel through flight, settings and rematch.
4. **Non-English layout:** bindings and prompts on at least one AZERTY/QWERTZ
   layout.
5. **Low vision:** 200% UI, high contrast, minimum-quality world readability.
6. **Hearing alternative:** muted game with captions and visual threat/result
   cues.
7. **Photosensitivity:** no-flash mode through storm, damage and spell bursts.
8. **Motion comfort:** expressive, steady and minimal profiles through skim,
   turn, climb, dive, impact and death/recovery.
9. **Blind/assistive-tech audit:** screen-reader completion of landing,
   settings, lobby and error recovery; document 3D-flight blockers rather than
   claiming unsupported blind play.
10. **One-hand/switch-style audit:** single-key menu operation and digital
    gameplay alternatives; document remaining time/coordination barriers.
11. **Focus chaos:** Escape, Alt-Tab, hidden tab, fullscreen refusal,
    controller removal and return.
12. **Recovery:** stalled network, expired reconnect, corrupt cache and old
    build.

### Quantitative first-session gates

- New friend reaches controllable orientation bubble within the stack's cold
  link-to-control budget.
- Returning friend reaches control within the cached budget.
- Permission refusal requires no reload and no repeated prompt loop.
- Essential shell flow has no keyboard trap or unreachable action.
- Input is cleared on the same client frame as focus/capture loss and before
  another command sample.
- Safe start is reachable before 3D motion in one activation.
- All required settings persist across reload and rematch.
- Every essential cue passes the no-color-only/no-audio-only review.
- At least one test participant uses each certified input path before S1.

## Evidence and failure artifact

The UX gate retains:

- exact browser/build/content/rules IDs;
- semantic-shell accessibility report;
- first-run and rematch recordings;
- input-action map and settings schema versions;
- comfort/flash/caption profile screenshots;
- denied-permission and recovery traces;
- pseudolocalization/RTL captures;
- known limitation register with owner and milestone;
- no room token, IP address, raw input trace or inferred disability.

## Deferred, not forgotten

- full screen-reader-driven 3D flight;
- speech-to-text/text-to-speech communication;
- touch/mobile controls;
- platform accessibility-setting import;
- native console certification;
- VR comfort and input;
- account/cloud settings synchronization.

Deferral means no S1 claim, not permission to make the architecture
incompatible.

## Sources

- [W3C Web Content Accessibility Guidelines 2.2](https://www.w3.org/TR/WCAG22/)
- [W3C WCAG 2.2 Understanding documents](https://www.w3.org/WAI/WCAG22/Understanding/)
- [Xbox Accessibility Guidelines](https://learn.microsoft.com/en-us/gaming/accessibility/guidelines)
- [Xbox Accessibility Guideline 107: Input](https://learn.microsoft.com/en-us/gaming/accessibility/xbox-accessibility-guidelines/107)
- [Xbox Accessibility Guideline 112: UI navigation](https://learn.microsoft.com/en-us/xbox/accessibility/xbox-accessibility-guidelines/112)
- [MDN Pointer Lock API](https://developer.mozilla.org/en-US/docs/Web/API/Pointer_Lock_API)
- [MDN Fullscreen API](https://developer.mozilla.org/en-US/docs/Web/API/Fullscreen_API)
- [MDN Page Visibility API](https://developer.mozilla.org/en-US/docs/Web/API/Page_Visibility_API)
