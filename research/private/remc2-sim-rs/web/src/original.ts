import * as THREE from 'three';
import { decodeOriginalFrame, ORIGINAL_HEIGHT, ORIGINAL_WIDTH, OriginalSceneHistory, originalInput, markedSpellCycle, markedSpellHand,
  preferenceKeyCode, mapGestureAdmitted, MapMouseChord,
  type OriginalFrame, type IndexedBlendTables } from './original-adapter';
import { OriginalHud, type OriginalHudState, type SpellMenu } from './original-hud';
import { decodeRenderPublication } from './render-publication';
import { decodeWorldFrame, decodeTerrain, type WorldFrame } from './original-world-data';
import { OriginalWorldRenderer } from './original-world-renderer';
import { ModernWorldRenderer } from './modern-world-renderer';
import { decodeModernFrame } from './modern-runtime';
import { OriginalMediaPlayer, type MediaReply } from './original-media';
import { OriginalGameplayAudio, type GameplayVoice } from './original-audio';
import { OriginalSpeech, type ObjectiveSpeechCue } from './original-speech';

const API = '/original-api';
const modernRequested = new URLSearchParams(location.search).get('visual') === 'modern';
const DISPLAY_WIDTH = 640, DISPLAY_HEIGHT = 480;
const element = <T extends HTMLElement>(id: string): T => {
  const found = document.getElementById(id);
  if (!found) throw new Error(`Missing game control: ${id}`);
  return found as T;
};
const canvas = element<HTMLCanvasElement>('original');
const curtain = element('curtain'), message = element('message');
const playButton = element<HTMLButtonElement>('play'), pauseButton = element<HTMLButtonElement>('pause');
const speechButton = element<HTMLButtonElement>('speech');
const menuButton = element<HTMLButtonElement>('menu');
const keys = new Set<string>();
const mouseButtons = new Set<number>();
const mapMouseChord = new MapMouseChord();
const consumedCycleButtons = new Set<number>();
const spellMenuButtons = new Set<number>();
let spellControlHeld = false;
let configuredMapCode: string | null = null;
let spellCloseOnRelease = false;
let mouseX = 0, mouseY = 0, queuedCasts = 0;
let started = false, playing = false, busy = false, inputBusy = false;
let controlsPending = 0;
let spellMutationPending = false;
let informationPending = false, informationHeld = false;
let inputSequence = 0, controlVersion = 0;
let connected = false;
let leavingPage = false;
let polling = false;
let lastFrame: OriginalFrame | WorldFrame | null = null;
let terrainIdentity = '';
const emptyScene = new Uint8Array(64000), hudCoverage = new Uint8Array(640 * 480);
let tables: IndexedBlendTables | undefined;
let hud: OriginalHud;
let spellMenu: SpellMenu | null = null;
let nativeMenus = { map_open: false, spells_open: false, player_information_open: false };
let abandonOpen = false;
let hoveredSlot = 0, hoveredTier = 0, intentionalUnlock = false;
let touchHand: 'left' | 'right' = 'left';
let lastHudState: OriginalHudState | null = null;
let inspectedExit: number | null = null;
let inspectionGeneration = '';
let currentLevelName = 'Level 1';
let campaignPhase: string | null = null;
let loadedAssetRevision: number | undefined;
let campaignMedia: OriginalMediaPlayer | null = null;
const enterMissionKey = 'mc2-browser-enter-mission';
const history = new OriginalSceneHistory();
const gameplayAudio = new OriginalGameplayAudio(API, row => {
  void post('/control', { action: 'audio_complete', generation: row.generation,
    voice: row.voice, event_sequence: row.sequence }).catch(error => console.error('Gameplay voice completion:', error));
});
const objectiveSpeech = new OriginalSpeech(API, () => gameplayAudio.audioContext(), action => gameplayAudio.speechMix(action), cue => {
  void post('/control', { action: 'speech_complete', generation: cue.generation,
    event_sequence: cue.sequence }).catch(error => console.error('Objective speech completion:', error));
});

function updateMapInspectionLabel(text: string | null): void {
  const label = text ? `Map inspection: ${text}` : 'Map inspection: click or tap an exit marker. Hold Alt for player information.';
  canvas.setAttribute('aria-label', label);
  element('hint').textContent = label;
}

function levelName(levelId: string | null | undefined): string {
  const slug = levelId?.match(/^level\.magic_carpet_2\.([a-z0-9_]+)$/)?.[1];
  if (!slug) return 'Level 1';
  if (slug === 't_klom') return "T’Klom";
  if (slug === 'night_spire') return 'NightSpire';
  return slug.split('_').map(word => word.charAt(0).toUpperCase() + word.slice(1)).join(' ');
}

class IndexedDisplay {
  private readonly renderer = new THREE.WebGLRenderer({ canvas, antialias: false, alpha: false,
    powerPreference: 'high-performance' });
  private readonly scene = new THREE.Scene();
  private readonly camera = new THREE.Camera();
  private readonly indices = new THREE.DataTexture(new Uint8Array(DISPLAY_WIDTH * DISPLAY_HEIGHT), DISPLAY_WIDTH, DISPLAY_HEIGHT,
    THREE.RedFormat, THREE.UnsignedByteType);
  private readonly palette = new THREE.DataTexture(new Uint8Array(1024), 256, 1, THREE.RGBAFormat, THREE.UnsignedByteType);
  private readonly paletteBytes = new Uint8Array(1024);
  readonly gpu: string;

  constructor() {
    this.renderer.setPixelRatio(1); this.renderer.setSize(DISPLAY_WIDTH, DISPLAY_HEIGHT, false);
    this.renderer.toneMapping = THREE.NoToneMapping;
    for (const texture of [this.indices, this.palette]) {
      texture.minFilter = THREE.NearestFilter; texture.magFilter = THREE.NearestFilter;
      texture.colorSpace = THREE.NoColorSpace; texture.generateMipmaps = false;
    }
    const material = new THREE.RawShaderMaterial({ glslVersion: THREE.GLSL3, depthTest: false, depthWrite: false,
      blending: THREE.NoBlending, toneMapped: false,
      uniforms: { sourceIndices: { value: this.indices }, sourcePalette: { value: this.palette } },
      vertexShader: `precision highp float;
        in vec3 position; in vec2 uv; out vec2 sourceUv;
        void main(){ sourceUv=uv; gl_Position=vec4(position.xy,0.,1.); }`,
      fragmentShader: `precision highp float;
        uniform sampler2D sourceIndices; uniform sampler2D sourcePalette;
        in vec2 sourceUv; out vec4 pixel;
        void main(){
          ivec2 p=clamp(ivec2(sourceUv.x*640.,(1.-sourceUv.y)*480.),ivec2(0),ivec2(639,479));
          int index=int(floor(texelFetch(sourceIndices,p,0).r*255.+.5));
          pixel=vec4(texelFetch(sourcePalette,ivec2(index,0),0).rgb,1.);
        }`,
    });
    this.scene.add(new THREE.Mesh(new THREE.PlaneGeometry(2, 2), material));
    const gl = this.renderer.getContext(); const info = gl.getExtension('WEBGL_debug_renderer_info');
    this.gpu = info ? String(gl.getParameter(info.UNMASKED_RENDERER_WEBGL)) : String(gl.getParameter(gl.RENDERER));
    this.renderer.render(this.scene, this.camera);
  }

  present(indices: Uint8Array, palette: Uint8Array): void {
    if (indices.length !== DISPLAY_WIDTH * DISPLAY_HEIGHT) throw new Error('Incomplete Original display composition');
    (this.indices.image.data as Uint8Array).set(indices);
    for (let i = 0; i < 256; i++) {
      this.paletteBytes[i * 4] = palette[i * 3]!; this.paletteBytes[i * 4 + 1] = palette[i * 3 + 1]!;
      this.paletteBytes[i * 4 + 2] = palette[i * 3 + 2]!; this.paletteBytes[i * 4 + 3] = 255;
    }
    (this.palette.image.data as Uint8Array).set(this.paletteBytes);
    this.indices.needsUpdate = true; this.palette.needsUpdate = true;
    this.renderer.render(this.scene, this.camera);
  }
}

type GeometryDisplay = OriginalWorldRenderer | ModernWorldRenderer;
function isGeometryDisplay(value: IndexedDisplay | GeometryDisplay): value is GeometryDisplay {
  return value instanceof OriginalWorldRenderer || value instanceof ModernWorldRenderer;
}
interface BridgeStatus {
  ready: boolean;
  level_id?: string | null;
  renderer?: 'indexed' | 'geometry';
  error?: string | null;
  running?: boolean;
  paused?: boolean;
  next_sequence?: number;
  has_save?: boolean;
  phase?: string;
  asset_revision?: number;
  can_refresh_assets?: boolean;
  speech_enabled?: boolean | null;
  speech_available?: boolean;
  campaign_menu?: {
    phase: 'results' | 'mission_select';
    completed_level_id?: string;
    selected_level_id?: string;
    available_levels: string[];
    completion_statistics?: {
      spells_found_percent: number; hit_accuracy_percent: number; creatures_killed_percent: number;
      mana_collected_percent: number; time_taken_seconds: number;
    };
  } | null;
}
interface BridgeState extends OriginalHudState {
  sequence: number | string;
  asset_revision?: number;
  runtime_generation: number | string;
  session_generation: number | string;
  tick: number | string;
  paused: boolean;
  show_help?: boolean;
  sound_enabled?: boolean;
  sound_volume?: number;
  map_key?: string | null;
  gameplay_audio?: GameplayVoice[];
  objective_speech?: ObjectiveSpeechCue[];
  speech_enabled?: boolean;
  speech_available?: boolean;
  campaign_effects?: {generation: string; sequence: string; kind: number; code: number; value0: number; value1: number}[];
  hint_entities?: { entity: number; x: number; y: number }[];
  source_menu_admission?: { map_open: boolean; spells_open: boolean; player_information_open: boolean } | null;
  source_menu_state?: number;
  objective?: { is_level_end: number; text_selector: number } | null;
  player_pose: { x: number; y: number; z: number; yaw: number };
}

async function responseJson<T>(response: Response): Promise<T> {
  const value = await response.json();
  if (!response.ok) throw new Error(typeof value.error === 'string' ? value.error : value.error?.message ?? `Game request failed (${response.status})`);
  return value as T;
}

async function readStatus(): Promise<BridgeStatus> {
  return responseJson<BridgeStatus>(await fetch(API + '/status', { cache: 'no-store', signal: AbortSignal.timeout(10000) }));
}

function showCampaign(status: BridgeStatus): boolean {
  const phase = status.can_refresh_assets ? 'refresh_assets' : status.phase;
  if (!phase || phase === 'playing' || phase === 'paused') return false;
  campaignPhase = phase;
  gameplayAudio.stop();
  objectiveSpeech.stop();
  abandonOpen = false; element('abandon-dialog').hidden = true;
  playing = false;
  polling = false;
  clearInput();
  if (isGeometryDisplay(display)) display.setPlaying(false);
  if (document.pointerLockElement === canvas) document.exitPointerLock();
  for (const id of ['controls', 'play', 'pause', 'save', 'load', 'reset', 'speech', 'abandon', 'menu', 'touch', 'look-hint', 'hint', 'retry']) element(id).hidden = true;
  if (phase === 'media') {
    curtain.hidden = true;
    if (!campaignMedia) {
      const player = new OriginalMediaPlayer(element('stage'), API, async (elapsed, input, escape) =>
        await post('/control', { action: 'media_frame', elapsed_us: elapsed, input, escape }) as MediaReply);
      campaignMedia = player;
      void player.run().then(async () => {
        if (campaignMedia !== player) return;
        campaignMedia = null;
        showCampaign(await readStatus());
      }).catch(error => {
        if (campaignMedia !== player) return;
        campaignMedia = null;
        curtain.hidden = false; element('campaign').hidden = false;
        element('campaign-continue').hidden = true; element('campaign-picker').hidden = true;
        const retry = element<HTMLButtonElement>('campaign-enter');
        retry.hidden = false; retry.disabled = false; retry.textContent = 'Retry intermission';
        message.textContent = error instanceof Error ? error.message : String(error);
      });
    }
    return true;
  }
  curtain.hidden = false;
  element('campaign').hidden = false;
  message.textContent = status.error ?? '';
  const menu = status.campaign_menu;
  const results = menu?.phase === 'results' && status.phase === 'results';
  const select = menu?.phase === 'mission_select' && status.phase === 'mission_select';
  const heading = document.querySelector('h2')!;
  heading.textContent = results ? `${levelName(menu.completed_level_id)} · Results` :
    select ? 'Select mission' : phase === 'refresh_assets' ? 'Mission assets unavailable' :
    phase === 'loading' ? 'Loading mission' : phase;
  document.title = `Magic Carpet II · ${heading.textContent}`;
  const stats = element('campaign-stats');
  stats.hidden = !results;
  stats.replaceChildren();
  if (results && menu.completion_statistics) {
    const row = menu.completion_statistics;
    const seconds = row.time_taken_seconds;
    for (const [name, value] of [
      ['Spells found', `${row.spells_found_percent}%`], ['Hit accuracy', `${row.hit_accuracy_percent}%`],
      ['Creatures killed', `${row.creatures_killed_percent}%`], ['Mana collected', `${row.mana_collected_percent}%`],
      ['Time taken', `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`]
    ] as const) {
      const term = document.createElement('dt'), detail = document.createElement('dd');
      term.textContent = name; detail.textContent = value; stats.append(term, detail);
    }
  }
  const continueButton = element<HTMLButtonElement>('campaign-continue');
  continueButton.hidden = !results && phase !== 'title';
  continueButton.textContent = results ? 'Continue' : 'Campaign map';
  element('campaign-picker').hidden = !select;
  const enter = element<HTMLButtonElement>('campaign-enter');
  enter.hidden = !select && phase !== 'loading' && phase !== 'refresh_assets';
  enter.textContent = select ? 'Enter mission' : 'Retry loading mission';
  enter.disabled = false;
  if (select) {
    const picker = element<HTMLSelectElement>('campaign-level');
    picker.replaceChildren(...menu.available_levels.map(id => new Option(levelName(id), id, false, id === menu.selected_level_id)));
    enter.disabled = !picker.value;
  }
  return true;
}

async function advanceCampaign(): Promise<void> {
  if (busy) return;
  controlsPending++; busy = true; controlVersion++;
  try {
    if (campaignPhase === 'media') {
      showCampaign(await readStatus());
    } else if (campaignPhase === 'results') {
      await post('/control', { action: 'dismiss_results' });
      showCampaign(await readStatus());
    } else if (campaignPhase === 'title') {
      await post('/control', { action: 'open_campaign' });
      showCampaign(await readStatus());
    } else if (campaignPhase === 'mission_select' || campaignPhase === 'loading' || campaignPhase === 'refresh_assets') {
      if (campaignPhase === 'mission_select') {
        const chosen = element<HTMLSelectElement>('campaign-level').value;
        const status = await readStatus();
        if (chosen !== status.campaign_menu?.selected_level_id)
          await post('/control', { action: 'select_mission', level_id: chosen });
      }
      message.textContent = 'Loading mission…';
      await post('/control', { action: campaignPhase === 'refresh_assets' ? 'refresh_assets' : 'start_mission' });
      const status = await readStatus();
      if (!showCampaign(status)) {
        // Replace the renderer and all package-bound assets together.
        sessionStorage.setItem(enterMissionKey, JSON.stringify([status.level_id, status.asset_revision]));
        location.reload();
      }
    }
  } catch (error) {
    try { showCampaign(await readStatus()); } catch { /* Keep the current controls available for a retry. */ }
    message.textContent = error instanceof Error ? error.message : String(error);
  } finally { controlsPending--; busy = controlsPending > 0; }
}

let requestTail: Promise<unknown> = Promise.resolve();
function post(path: string, fields: Record<string, unknown>): Promise<Record<string, unknown>> {
  const request = async (): Promise<Record<string, unknown>> => {
    const body = JSON.stringify({ sequence: inputSequence, ...fields });
    let networkError: unknown;
    // A retry retains the same sequence and exact body. The backend owns
    // admission/deduplication; it must never execute a repeated cast twice.
    for (let attempt = 0; attempt < 2; attempt++) {
      let response: Response;
      try { response = await fetch(API + path, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body,
        signal: AbortSignal.timeout(fields.action === 'start_mission' || fields.action === 'refresh_assets' ? 180000 : 8000) }); }
      catch (error) { networkError = error; continue; }
      const value = await responseJson<Record<string, unknown>>(response);
      inputSequence++; return value;
    }
    throw networkError ?? new Error('The local game connection was lost');
  };
  const next = requestTail.then(request); requestTail = next.catch(() => undefined); return next;
}

function clearInput(): void { keys.clear(); mouseButtons.clear(); consumedCycleButtons.clear(); mouseX = 0; mouseY = 0; queuedCasts = 0; }
function showSpeechPreference(enabled: unknown, available: unknown): void {
  speechButton.disabled = available !== true || typeof enabled !== 'boolean';
  speechButton.textContent = speechButton.disabled ? 'Speech unavailable' : `Speech: ${enabled ? 'On' : 'Off'}`;
}
function showMenu(text: string): void {
  message.textContent = text; curtain.hidden = false;
  playButton.hidden = started; pauseButton.hidden = !started;
  for (const id of ['save', 'load', 'reset', 'abandon']) element(id).hidden = !started;
  speechButton.hidden = !connected;
  menuButton.hidden = !started;
  element('touch').hidden = true;
  element('look-hint').hidden = true;
}
function hideMenu(): void {
  curtain.hidden = true; menuButton.hidden = false; element('hint').hidden = false; element('touch').hidden = false;
  element('hint').textContent = 'Shift+click: next marked spell · Alt+click: previous · Hold Alt on map: player information · Ctrl: spells';
  element('look-hint').hidden = false;
}
function fail(error: unknown): void {
  gameplayAudio.stop();
  objectiveSpeech.stop();
  // Navigation cancels pending HTTP reads. It is not a failure of the next page.
  if (leavingPage) return;
  playing = false; connected = false; clearInput();
  if (isGeometryDisplay(display)) display.setPlaying(false);
  showMenu(error instanceof Error ? error.message : String(error));
  element('retry').hidden = false;
  console.error(error);
}

async function control(action: string, fields: Record<string, unknown> = {}): Promise<void> {
  const spellMutation = action === 'mark_spell' || action === 'cycle_marked_spell';
  const informationMutation = action === 'menu' && (fields.menu_action === 'show_player_information' || fields.menu_action === 'hide_player_information');
  if (spellMutation && spellMutationPending) return;
  if (spellMutation) spellMutationPending = true;
  if (action === 'pause' || action === 'load' || action === 'reset') gameplayAudio.stop();
  if (action === 'load' || action === 'reset') objectiveSpeech.stop();
  controlsPending++; busy = true; controlVersion++;
  if (['start', 'pause', 'resume', 'load', 'reset'].includes(action)) {
    lastArrival = 0; intervals.length = 0;
  }
  if (!spellMutation && !informationMutation) clearInput();
  try {
    const result = await post('/control', { action, ...fields });
    if (action === 'start' || action === 'resume') {
      started = true; playing = true; hideMenu(); canvas.focus();
      if (!polling) { polling = true; void pollFrame(); }
    }
    else if (action === 'pause') { playing = false; showMenu('Paused'); }
    else if (action === 'save') { message.textContent = 'Game saved.'; element('hint').textContent = 'Game saved.'; }
    else if (action === 'toggle_speech') showSpeechPreference(result.speech_enabled, result.speech_available);
    else if (action === 'load' || action === 'reset') {
      history.reset(); playing = result.running === true; showMenu(action === 'load' ? 'Game loaded.' : 'Level restarted.');
      if (playing) hideMenu();
    }
    else if (action === 'menu' && result.phase === 'title') showCampaign(await readStatus());
    if (action !== 'start' && !spellMutation && !informationMutation && document.pointerLockElement === canvas) { intentionalUnlock = true; document.exitPointerLock(); }
    if (isGeometryDisplay(display)) display.setPlaying(playing);
  } catch (error) {
    if (spellMutation) spellMutationPending = false;
    if (informationMutation) informationPending = false;
    // A refused save/selection leaves the existing native owner intact.
    message.textContent = error instanceof Error ? error.message : String(error);
    if (!playing) showMenu(message.textContent);
    else { element('hint').textContent = message.textContent; element('hint').hidden = false; }
  }
  finally { controlsPending--; busy = controlsPending > 0; }
}

function syncPlayerInformation(): void {
  if (!playing || busy || informationPending || abandonOpen || !nativeMenus.map_open || nativeMenus.spells_open) return;
  if (informationHeld === nativeMenus.player_information_open) return;
  informationPending = true;
  void control('menu', { menu_action: informationHeld ? 'show_player_information' : 'hide_player_information' });
}

let display: IndexedDisplay | GeometryDisplay;
let packetCount = 0, bytesReceived = 0, lastArrival = 0;
const intervals: number[] = [];
async function pollFrame(): Promise<void> {
  const version = controlVersion;
  const before = performance.now();
  try {
    // A lifecycle request may be queued behind an input POST. Do not start a
    // read in that window: it can return the previous paused publication after
    // resume has already acknowledged. Older reads are rejected by version.
    if (busy) return;
    const geometry = isGeometryDisplay(display);
    const response = await fetch(API + (geometry ? '/world-frame' : '/frame'), { cache: 'no-store', signal: AbortSignal.timeout(10000) });
    if (response.status === 204) {
      const status = await readStatus();
      // A direct secret entry briefly has no frames while the owner binds the new mission.
      if (version === controlVersion && (status.phase !== 'loading' || status.can_refresh_assets)) showCampaign(status);
      return;
    }
    if (response.status === 503) {
      const status = await readStatus();
      if (version !== controlVersion || (status.can_refresh_assets && showCampaign(status))) return;
    }
    if (!response.ok) { await responseJson(response); return; }
    const data = await response.arrayBuffer();
    const frame = geometry ? decodeWorldFrame(data) : decodeOriginalFrame(data);
    if (version !== controlVersion) return;
    if (lastFrame && frame.runtimeGeneration === lastFrame.runtimeGeneration && frame.sequence <= lastFrame.sequence) return;
    const query = '?sequence=' + frame.sequence;
    const [stateResponse, observationResponse, modernResponse] = await Promise.all([
      fetch(API + '/state' + query, { cache: 'no-store', signal: AbortSignal.timeout(10000) }),
      fetch(API + '/observation' + query, { cache: 'no-store', signal: AbortSignal.timeout(10000) }),
      display instanceof ModernWorldRenderer ? fetch(API + '/modern-frame' + query, { cache: 'no-store', signal: AbortSignal.timeout(10000) }) : Promise.resolve(null),
    ]);
    if (stateResponse.status === 410 || observationResponse.status === 410 || modernResponse?.status === 410) return;
    const state = await responseJson<BridgeState>(stateResponse);
    if (version !== controlVersion) return;
    showSpeechPreference(state.speech_enabled, state.speech_available);
    configuredMapCode = preferenceKeyCode(state.map_key);
    if (state.asset_revision !== undefined && loadedAssetRevision !== undefined && state.asset_revision !== loadedAssetRevision) {
      // A native secret entry or a loaded slot can change the mission without a menu.
      const status = await readStatus();
      if (version !== controlVersion || showCampaign(status)) return;
      connected = false; playing = false;
      if (status.running) sessionStorage.setItem(enterMissionKey, JSON.stringify([status.level_id, status.asset_revision]));
      location.reload(); return;
    }
    if (!observationResponse.ok) { await responseJson(observationResponse); return; }
    const observation = decodeRenderPublication(new Uint8Array(await observationResponse.arrayBuffer()));
    if (observation.runtime_generation !== frame.runtimeGeneration || observation.identity.generation !== frame.sessionGeneration ||
        observation.identity.tick !== frame.tick || BigInt(state.sequence) !== frame.sequence ||
        BigInt(state.runtime_generation) !== frame.runtimeGeneration || BigInt(state.session_generation) !== frame.sessionGeneration || BigInt(state.tick) !== frame.tick)
      throw new Error('The original scene and HUD do not describe the same native frame');
    if (version !== controlVersion) return;
    lastFrame = frame;
    gameplayAudio.update(state.gameplay_audio ?? [], frame.sessionGeneration.toString(),
      state.asset_revision ?? 0, state.sound_enabled === true, playing && !state.paused, state.sound_volume ?? 0);
    objectiveSpeech.update(state.objective_speech ?? [], state.asset_revision ?? 0,
      state.sound_enabled === true, state.sound_volume ?? 0);
    spellMutationPending = false;
    informationPending = false;
    const wasMapOpen = nativeMenus.map_open, wasSpellsOpen = nativeMenus.spells_open;
    nativeMenus = state.source_menu_admission ?? { map_open: false, spells_open: false, player_information_open: false };
    const wasAbandonOpen = abandonOpen;
    abandonOpen = state.source_menu_state === 13 || state.source_menu_state === 14;
    element('abandon-dialog').hidden = !abandonOpen;
    if (abandonOpen && !wasAbandonOpen) { clearInput(); element<HTMLButtonElement>('abandon-cancel').focus(); }
    if (nativeMenus.spells_open !== wasSpellsOpen) {
      element('hint').textContent = nativeMenus.spells_open
        ? 'Shift+left/right click marks or unmarks a spell for that hand'
        : 'Shift+click: next marked spell · Alt+click: previous · Hold Alt on map: player information · Ctrl: spells';
    }
    if (nativeMenus.map_open && !wasMapOpen && !(display instanceof ModernWorldRenderer)) updateMapInspectionLabel(null);
    if (!nativeMenus.map_open && wasMapOpen && !(display instanceof ModernWorldRenderer)) {
      inspectedExit = null;
      canvas.setAttribute('aria-label', `${currentLevelName} game view`);
      element('hint').textContent = 'Shift+click: next marked spell · Alt+click: previous · Hold Alt on map: player information · Ctrl: spells';
    }
    document.body.dataset.gameMenu = element('touch').dataset.menu = nativeMenus.spells_open ? 'spells' : 'flight';
    element('touch-hand').hidden = !nativeMenus.spells_open;
    element('touch-spells').textContent = nativeMenus.spells_open ? 'Close spells' : 'Spells';
    const source = observation.value;
    spellMenu = { open: nativeMenus.spells_open, hovered_slot: hoveredSlot, hovered_tier: hoveredTier,
      mana: source.player_vitals.mana, wizard_colors: source.spell_menu_wizard_colors,
      panels_opaque: source.spell_menu_panels_opaque, spells: source.player_spells };
    const objectiveSelector = source.objective_secret_announcement_record ??
      (state.objective ? hud.objectiveRecord(state.objective.text_selector, state.objective.is_level_end !== 0) : null);
    const hudState: OriginalHudState = { ...state, source_top_status: source.source_top_status,
      panels_opaque: source.spell_menu_panels_opaque, objective_selector: objectiveSelector,
      objective_visible: source.objective_textbox_ticks > 0, spell_menu: spellMenu,
      player_information: nativeMenus.player_information_open ? source.source_scoreboard : null };
    hud.advanceAbilityNotification({generation: frame.sessionGeneration.toString(), sequence: frame.sequence.toString(),
      assetRevision: state.asset_revision ?? 0, effects: state.campaign_effects ?? [], suppressed: abandonOpen});
    if (state.objective) hudState.level_end = state.objective.is_level_end !== 0;
    const sourceGeneration = `${frame.runtimeGeneration}:${frame.sessionGeneration}`;
    if (sourceGeneration !== inspectionGeneration) { inspectedExit = null; inspectionGeneration = sourceGeneration; }
    if (display instanceof ModernWorldRenderer) hudState.world_map = undefined;
    hudState.exit_inspection_allowed = !!hudState.world_map && nativeMenus.map_open && !nativeMenus.spells_open &&
      state.objective?.is_level_end === 0 && source.objective_textbox_ticks === 0 && !source.exit_sequence_active;
    if (inspectedExit !== null && !hud.exitMarker(hudState, inspectedExit)) {
      inspectedExit = null;
      if (nativeMenus.map_open) updateMapInspectionLabel(null);
    }
    hudState.exit_inspection_subject = inspectedExit;
    lastHudState = hudState;
    const advanceHints = (entities: readonly { entity: number; x: number; y: number }[]) =>
      hud.advanceMapHints(hudState, { generation: sourceGeneration, sequence: frame.sequence.toString(),
        enabled: state.show_help === true, paused: state.paused,
        suppressed: abandonOpen || nativeMenus.spells_open || nativeMenus.player_information_open || source.player_vitals.life < 0 ||
          source.objective_textbox_ticks > 0 || source.exit_sequence_active || state.objective?.is_level_end !== 0,
        player: state.player_pose, entities: [...entities, ...(state.hint_entities ?? [])] });
    // Flight history is independent of map presentation and remains at the
    // correct native revision while the source split map is displayed.
    if (isGeometryDisplay(display)) {
      const worldFrame = frame as WorldFrame;
      const epoch = `${frame.runtimeGeneration}:${frame.sessionGeneration}:${worldFrame.terrainRevision}`;
      if (epoch !== terrainIdentity) {
        const terrainResponse = await fetch(API + '/terrain' + query, { signal: AbortSignal.timeout(10000) });
        if (terrainResponse.status === 410) return;
        if (!terrainResponse.ok) { await responseJson(terrainResponse); return; }
        const terrain = decodeTerrain(await terrainResponse.arrayBuffer());
        if (version !== controlVersion) return;
        display.updateTerrain(terrain); terrainIdentity = epoch;
      }
      if (state.minimap) hudState.minimap = { ...state.minimap, terrain: display.minimap(source.source_camera_pose) };
      if (nativeMenus.map_open) { hudState.minimap = undefined; hudState.top_bar_visible = false; }
      advanceHints(worldFrame.sprites);
      // Source art stays indexed. Opaque panels avoid flattening a translucent
      // HUD against an invented background; the 3D world is never read to CPU.
      hudState.panels_opaque = true;
      const hudScene = nativeMenus.map_open && display instanceof OriginalWorldRenderer
        ? display.fullMapTerrain(source.source_camera_pose) : emptyScene;
      const overlay = hud.compose(hudScene, frame.palette, hudState, hudCoverage);
      if (display instanceof ModernWorldRenderer) {
        if (!modernResponse?.ok) throw new Error('This backend has no Modern asset observations; start it with --modern.');
        const modern = decodeModernFrame(await modernResponse.arrayBuffer());
        if (version !== controlVersion) return;
        display.present(worldFrame, source.source_camera_pose, source.source_camera_fov, overlay, hudCoverage, nativeMenus.map_open, playing && !state.paused, modern);
        Object.assign(window, { modernState: modern });
      } else display.present(worldFrame, source.source_camera_pose, source.source_camera_fov, overlay, hudCoverage, nativeMenus.map_open, playing && !state.paused);
    } else {
    const sceneFrame = frame as OriginalFrame;
    const scene = history.compose(sceneFrame, tables);
    let visible = sceneFrame;
    if (nativeMenus.map_open) {
      const mapResponse = await fetch(API + '/world-map' + query, { cache: 'no-store', signal: AbortSignal.timeout(10000) });
      if (mapResponse.status === 410) return;
      if (!mapResponse.ok) { await responseJson(mapResponse); return; }
      visible = decodeOriginalFrame(await mapResponse.arrayBuffer(), 'map');
      if (visible.sequence !== frame.sequence || visible.runtimeGeneration !== frame.runtimeGeneration || visible.sessionGeneration !== frame.sessionGeneration)
        throw new Error('Original map identity differs from the native scene');
    }
    if (version !== controlVersion) return;
    advanceHints([]);
    display.present(hud.compose(nativeMenus.map_open ? visible.indices : scene, visible.palette, hudState), visible.palette);
    }
    Object.assign(window, { originalState: state, originalObservation: observation });
    syncPlayerInformation();
    if (started && !busy && source.source_menu_admission?.spells_open && document.pointerLockElement === canvas) {
      intentionalUnlock = true; document.exitPointerLock();
    }
    if (started && !busy && state.paused && playing) { playing = false; clearInput(); showMenu('Paused'); }
    const now = performance.now(); if (lastArrival) { intervals.push(now - lastArrival); if (intervals.length > 600) intervals.shift(); }
    lastArrival = now; packetCount++; bytesReceived += data.byteLength;
    const sorted = [...intervals].sort((a,b) => a-b);
    Object.assign(window, { originalMetrics: { gpu: display.gpu, render_tick: frame.tick.toString(),
      runtime_generation: frame.runtimeGeneration.toString(), session_generation: frame.sessionGeneration.toString(),
      sequence: frame.sequence.toString(), scene_revision: ('sceneRevision' in frame ? frame.sceneRevision : frame.terrainRevision).toString(), packets: packetCount,
      received_hz: intervals.length ? intervals.length * 1000 / intervals.reduce((a,b) => a+b, 0) : 0,
      delivery_p95_ms: sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * .95))] ?? 0,
      bytes_received: bytesReceived, source_width: ORIGINAL_WIDTH, source_height: ORIGINAL_HEIGHT,
      display_width: DISPLAY_WIDTH, display_height: DISPLAY_HEIGHT,
      playing, paused: !playing && started } });
  } catch (error) { fail(error); }
  finally { if (connected && !campaignPhase) window.setTimeout(() => void pollFrame(), Math.max(5, 1000 / 30 - (performance.now() - before))); }
}

async function sendInput(): Promise<void> {
  if (!playing || busy || inputBusy) return; inputBusy = true;
  const cycleModifier = ['ShiftLeft', 'ShiftRight', 'AltLeft', 'AltRight'].some(key => keys.has(key));
  if (cycleModifier) queuedCasts = 0;
  const cast = queuedCasts > 0;
  const command = originalInput(abandonOpen || nativeMenus.spells_open || nativeMenus.map_open ? new Set<string>() : keys,
    abandonOpen ? 0 : mouseX, abandonOpen ? 0 : mouseY,
    { selected: !abandonOpen && !cycleModifier && (cast || keys.has('Space')), left: !abandonOpen && !cycleModifier && !mapMouseChord.isConsumed(0) && mouseButtons.has(0), right: !abandonOpen && !cycleModifier && !mapMouseChord.isConsumed(2) && mouseButtons.has(2) });
  mouseX *= .65; mouseY *= .65;
  try { await post('/input', { command }); if (cast) queuedCasts = Math.max(0, queuedCasts - 1); }
  catch (error) { fail(error); }
  finally { inputBusy = false; }
}

playButton.addEventListener('click', () => { gameplayAudio.activate(); void control('start'); });
element('campaign-continue').addEventListener('click', () => void advanceCampaign());
element('campaign-enter').addEventListener('click', () => void advanceCampaign());
element('abandon-cancel').addEventListener('click', () => void control('menu', { menu_action: 'cancel_abandon_confirmation' }));
element('abandon-confirm').addEventListener('click', () => void control('menu', { menu_action: 'confirm_abandon' }));
element('abandon').addEventListener('click', () => void (async () => {
  if (!playing) await control('resume');
  if (playing) await control('menu', { menu_action: 'open_abandon_confirmation' });
})());
pauseButton.addEventListener('click', () => { gameplayAudio.activate(); void control('resume'); });
speechButton.addEventListener('click', () => void control('toggle_speech'));
menuButton.addEventListener('click', () => void control(playing ? 'pause' : 'resume'));
for (const id of ['save', 'load', 'reset']) element(id).addEventListener('click', () => void control(id));
element('retry').addEventListener('click', () => location.reload());
element('fullscreen').addEventListener('click', () => {
  if (document.fullscreenElement) void document.exitFullscreen();
  else void element('stage').requestFullscreen().catch(error => { message.textContent = String(error); });
});
canvas.addEventListener('contextmenu', event => event.preventDefault());
const touches = new Map<number, { x: number; y: number }>();
const hudPoint = (x: number, y: number): [number, number] => {
  if (isGeometryDisplay(display)) return display.hudPoint(x, y);
  const rect = canvas.getBoundingClientRect(); return [(x - rect.left) * 640 / rect.width, (y - rect.top) * 480 / rect.height];
};
canvas.addEventListener('pointerdown', event => {
  if (!playing || abandonOpen || (busy && !informationPending) || consumedCycleButtons.has(event.button)) return;
  gameplayAudio.activate();
  if (nativeMenus.spells_open && spellMenu) {
    event.preventDefault();
    if (event.button === 0 || event.button === 2) spellMenuButtons.add(event.button);
    const hit = hud.hitTest(...hudPoint(event.clientX, event.clientY), spellMenu);
    if (hit) { hoveredSlot = hit.hoveredSlot; hoveredTier = hit.hoveredTier; }
    if (event.shiftKey && hit && (event.button === 0 || event.button === 2)) {
      const spell = spellMenu.spells.find(row => row.menu_slot === hit.hoveredSlot);
      const hand = spell && markedSpellHand(event.button, spell.marked_hand);
      if (spell?.acquired && spell.holder_actor !== undefined && spell.menu_artwork && hand)
        void control('mark_spell', { spell_id: spell.id, hand, holder_actor: String(spell.holder_actor) });
      return;
    }
    if (hit?.spellId != null && hit.tier != null && (event.button === 0 || event.button === 2))
      void control('assign_spell', { spell_id: hit.spellId, tier: hit.tier, hand: event.pointerType === 'touch' ? touchHand : event.button === 0 ? 'left' : 'right' });
    return;
  }
  const cycle = markedSpellCycle(event.button, event.shiftKey, event.altKey);
  if (cycle) {
    event.preventDefault(); consumedCycleButtons.add(event.button); mouseButtons.delete(event.button);
    queuedCasts = 0;
    void control('cycle_marked_spell', cycle);
    return;
  }
  if (nativeMenus.map_open && !(display instanceof ModernWorldRenderer)) {
    const subject = lastHudState ? hud.exitMarkerAt(lastHudState, ...hudPoint(event.clientX, event.clientY)) : null;
    inspectedExit = subject === inspectedExit ? null : subject;
    const text = inspectedExit && lastHudState ? hud.exitInspectionText(lastHudState, inspectedExit) : null;
    updateMapInspectionLabel(text);
    return;
  }
  canvas.focus();
  if (event.pointerType === 'touch') {
    event.preventDefault(); canvas.setPointerCapture(event.pointerId);
    touches.set(event.pointerId, { x: event.clientX, y: event.clientY }); return;
  }
  if (event.pointerType === 'mouse' && !new URLSearchParams(location.search).has('automation') && document.pointerLockElement !== canvas) {
    void canvas.requestPointerLock().catch(() => undefined);
    return;
  }
  mouseButtons.add(event.button);
});
// PointerEvent.pointerdown fires only for the first held mouse button; the
// source map gesture needs the second physical button's mousedown edge too.
canvas.addEventListener('mousedown', event => {
  if (!playing || abandonOpen || busy || nativeMenus.spells_open || (event.button !== 0 && event.button !== 2)) return;
  if (!nativeMenus.map_open && !new URLSearchParams(location.search).has('automation') && document.pointerLockElement !== canvas) return;
  if (mapMouseChord.press(event.button, { mapOpen: nativeMenus.map_open, spellsOpen: nativeMenus.spells_open,
    controlHeld: spellControlHeld || event.ctrlKey, cycleHeld: event.shiftKey || event.altKey })) {
    event.preventDefault(); mouseButtons.delete(0); mouseButtons.delete(2); queuedCasts = 0;
    void control('map');
  }
});
canvas.addEventListener('pointermove', event => {
  const prior = touches.get(event.pointerId);
  if (!prior || !playing || nativeMenus.spells_open) return;
  mouseX = Math.max(-64, Math.min(64, mouseX + (event.clientX - prior.x) * .65));
  mouseY = Math.max(-64, Math.min(64, mouseY + (event.clientY - prior.y) * .65));
  prior.x = event.clientX; prior.y = event.clientY;
});
canvas.addEventListener('pointerup', event => touches.delete(event.pointerId));
canvas.addEventListener('pointercancel', event => touches.delete(event.pointerId));
let hadPointerLock = false;
document.addEventListener('pointerlockchange', () => {
  const locked = document.pointerLockElement === canvas;
  if (!locked && hadPointerLock && playing && !busy && !intentionalUnlock) void control('pause');
  intentionalUnlock = false;
  hadPointerLock = locked;
});
const releaseButton = (event: PointerEvent): void => {
  mouseButtons.delete(event.button); consumedCycleButtons.delete(event.button);
  if (spellMenuButtons.delete(event.button) && !spellMenuButtons.size && spellCloseOnRelease && playing && nativeMenus.spells_open) {
    spellCloseOnRelease = false;
    void control('menu', { menu_action: 'close_spell_menu' });
  }
};
window.addEventListener('pointerup', releaseButton);
window.addEventListener('pointercancel', releaseButton);
window.addEventListener('mouseup', event => mapMouseChord.release(event.button));
window.addEventListener('mousemove', event => {
  if (playing && nativeMenus.spells_open && spellMenu) {
    const hit = hud.hitTest(...hudPoint(event.clientX, event.clientY), spellMenu);
    if (hit) { hoveredSlot = hit.hoveredSlot; hoveredTier = hit.hoveredTier; }
    return;
  }
  if (playing && document.pointerLockElement === canvas) {
    mouseX = Math.max(-64, Math.min(64, mouseX + event.movementX * .42));
    mouseY = Math.max(-64, Math.min(64, mouseY + event.movementY * .34));
  }
});
window.addEventListener('keydown', event => {
  if (campaignPhase || !started || event.target instanceof HTMLInputElement || event.target instanceof HTMLSelectElement) return;
  if (abandonOpen) { if (event.code === 'Escape') { event.preventDefault(); void control('menu', { menu_action: 'cancel_abandon_confirmation' }); } return; }
  if (!playing && !['Escape','KeyP','F5','F9'].includes(event.code)) return;
  gameplayAudio.activate();
  const mapKey = event.code === 'Tab' || event.code === configuredMapCode;
  if (mapKey || ['Space','ArrowLeft','ArrowRight','ArrowUp','ArrowDown','Escape','F5','F9','AltLeft','AltRight'].includes(event.code)) event.preventDefault();
  if (mapKey && mapGestureAdmitted({ mapOpen: nativeMenus.map_open, spellsOpen: nativeMenus.spells_open,
    controlHeld: spellControlHeld || event.ctrlKey, cycleHeld: event.shiftKey || event.altKey })) {
    if (!event.repeat && !busy) void control('map');
    return;
  }
  keys.add(event.code);
  if (event.repeat) return;
  if (event.code === 'Space') queuedCasts = Math.min(4, queuedCasts + 1);
  if (event.code === 'Escape' && playing && !nativeMenus.spells_open) void control('menu', { menu_action: 'open_abandon_confirmation' });
  else if (event.code === 'KeyP' || (event.code === 'Escape' && !playing)) void control(playing ? 'pause' : 'resume');
  if (event.code === 'ControlLeft' || event.code === 'ControlRight') { spellControlHeld = true; spellCloseOnRelease = false; void control('menu', { menu_action: 'open_spell_menu' }); }
  if (event.code === 'AltLeft' || event.code === 'AltRight') { informationHeld = true; syncPlayerInformation(); }
  if (event.code === 'F5') void control('save');
  if (event.code === 'F9') void control('load');
});
window.addEventListener('keyup', event => {
  keys.delete(event.code);
  if (event.code === 'AltLeft' || event.code === 'AltRight') { informationHeld = event.altKey; syncPlayerInformation(); }
  if (event.code === 'ControlLeft' || event.code === 'ControlRight') {
    spellControlHeld = event.ctrlKey;
    if (playing && !spellControlHeld) {
      if (spellMenuButtons.size) spellCloseOnRelease = true;
      else void control('menu', { menu_action: 'close_spell_menu' });
    }
  }
});
window.addEventListener('blur', () => { informationHeld = false; spellControlHeld = false; spellCloseOnRelease = false; spellMenuButtons.clear(); mapMouseChord.clear(); clearInput(); if (playing) void control('pause'); });
window.addEventListener('beforeunload', () => { objectiveSpeech.stop(); gameplayAudio.stop(); leavingPage = true; connected = false; playing = false; clearInput(); });
document.addEventListener('visibilitychange', () => { if (document.hidden) { mapMouseChord.clear(); clearInput(); if (playing) void control('pause'); } });
element('touch-map').addEventListener('click', () => void control('map'));
element('touch-hand').addEventListener('click', () => { touchHand = touchHand === 'left' ? 'right' : 'left'; element('touch-hand').textContent = touchHand === 'left' ? 'Left hand' : 'Right hand'; });
element('touch-spells').addEventListener('click', () => void control('menu', { menu_action: nativeMenus.spells_open ? 'close_spell_menu' : 'open_spell_menu' }));
for (const button of document.querySelectorAll<HTMLButtonElement>('[data-key],[data-cast]')) {
  button.addEventListener('pointerdown', event => {
    event.preventDefault(); button.setPointerCapture(event.pointerId);
    if (button.dataset.key) keys.add(button.dataset.key); else { keys.add('Space'); queuedCasts = Math.min(4, queuedCasts + 1); }
  });
  const release = () => { if (button.dataset.key) keys.delete(button.dataset.key); else keys.delete('Space'); };
  button.addEventListener('pointerup', release); button.addEventListener('pointercancel', release);
}

async function boot(): Promise<void> {
  try {
    let status: BridgeStatus | null = null;
    for (let attempt = 0; attempt < 600; attempt++) {
      status = await readStatus();
      if (status.can_refresh_assets || (status.ready && !(status.phase === 'loading' && status.running))) break;
      if (status.error) throw new Error(status.error);
      await new Promise(resolve => window.setTimeout(resolve, 300));
    }
    if (!status?.ready && !status?.can_refresh_assets) throw new Error('The original game did not finish loading.');
    inputSequence = status.next_sequence ?? 0;
    connected = true;
    if (showCampaign(status)) return;
    showSpeechPreference(status.speech_enabled, status.speech_available);
    speechButton.hidden = false;
    loadedAssetRevision = status.asset_revision;
    currentLevelName = levelName(status.level_id);
    const presentationName = modernRequested ? 'Modern' : 'Original';
    document.title = `Magic Carpet II · ${presentationName} ${currentLevelName}`;
    document.querySelector('h2')!.textContent = `${presentationName} · ${currentLevelName}`;
    playButton.textContent = `Play ${currentLevelName}`;
    canvas.setAttribute('aria-label', `${currentLevelName} game view`);
    if (status.renderer === 'geometry') {
      document.body.dataset.renderer = 'geometry';
      if (modernRequested) {
        message.textContent = `Loading the ${currentLevelName} models and environment…`;
        display = await ModernWorldRenderer.load(canvas, API);
      } else display = await OriginalWorldRenderer.load(canvas, API);
      display.onError = fail;
    } else display = new IndexedDisplay();
    hud = await OriginalHud.load(API); tables = hud.tables;
    if (display instanceof ModernWorldRenderer) {
      polling = true; await pollFrame();
      if (!connected) return;
      await display.ready;
    }
    playButton.disabled = false; message.textContent = modernRequested ? 'The world is ready.' : 'The original world is ready.';
    window.setInterval(() => void sendInput(), 1000 / 30);
    const enter = sessionStorage.getItem(enterMissionKey);
    sessionStorage.removeItem(enterMissionKey);
    if (enter && enter === JSON.stringify([status.level_id, status.asset_revision])) await control('start');
  } catch (error) { fail(error); }
}
void boot();
