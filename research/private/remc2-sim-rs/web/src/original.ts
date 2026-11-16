import * as THREE from 'three';
import { decodeOriginalFrame, ORIGINAL_HEIGHT, ORIGINAL_WIDTH, OriginalSceneHistory, originalInput,
  type OriginalFrame, type IndexedBlendTables } from './original-adapter';
import { OriginalHud, type OriginalHudState, type SpellMenu } from './original-hud';
import { decodeRenderPublication } from './render-publication';
import { decodeWorldFrame, decodeTerrain, type WorldFrame } from './original-world-data';
import { OriginalWorldRenderer } from './original-world-renderer';

const API = '/original-api';
const DISPLAY_WIDTH = 640, DISPLAY_HEIGHT = 480;
const element = <T extends HTMLElement>(id: string): T => {
  const found = document.getElementById(id);
  if (!found) throw new Error(`Missing game control: ${id}`);
  return found as T;
};
const canvas = element<HTMLCanvasElement>('original');
const curtain = element('curtain'), message = element('message');
const playButton = element<HTMLButtonElement>('play'), pauseButton = element<HTMLButtonElement>('pause');
const menuButton = element<HTMLButtonElement>('menu');
const keys = new Set<string>();
const mouseButtons = new Set<number>();
let mouseX = 0, mouseY = 0, queuedCasts = 0;
let started = false, playing = false, busy = false, inputBusy = false;
let controlsPending = 0;
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
let nativeMenus = { map_open: false, spells_open: false };
let hoveredSlot = 0, hoveredTier = 0, intentionalUnlock = false;
let touchHand: 'left' | 'right' = 'left';
const history = new OriginalSceneHistory();

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

interface BridgeStatus {
  ready: boolean;
  renderer?: 'indexed' | 'geometry';
  error?: string | null;
  running?: boolean;
  paused?: boolean;
  next_sequence?: number;
  has_save?: boolean;
}
interface BridgeState extends OriginalHudState {
  sequence: number | string;
  runtime_generation: number | string;
  session_generation: number | string;
  tick: number | string;
  paused: boolean;
  source_menu_admission?: { map_open: boolean; spells_open: boolean } | null;
  objective?: { is_level_end: number; text_selector: number } | null;
  player_pose: { x: number; y: number; z: number; yaw: number };
}

async function responseJson<T>(response: Response): Promise<T> {
  const value = await response.json();
  if (!response.ok) throw new Error(typeof value.error === 'string' ? value.error : value.error?.message ?? `Game request failed (${response.status})`);
  return value as T;
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
        signal: AbortSignal.timeout(8000) }); }
      catch (error) { networkError = error; continue; }
      const value = await responseJson<Record<string, unknown>>(response);
      inputSequence++; return value;
    }
    throw networkError ?? new Error('The local game connection was lost');
  };
  const next = requestTail.then(request); requestTail = next.catch(() => undefined); return next;
}

function clearInput(): void { keys.clear(); mouseButtons.clear(); mouseX = 0; mouseY = 0; queuedCasts = 0; }
function showMenu(text: string): void {
  message.textContent = text; curtain.hidden = false;
  playButton.hidden = started; pauseButton.hidden = !started;
  for (const id of ['save', 'load', 'reset']) element(id).hidden = !started;
  menuButton.hidden = !started;
  element('touch').hidden = true;
  element('look-hint').hidden = true;
}
function hideMenu(): void {
  curtain.hidden = true; menuButton.hidden = false; element('hint').hidden = false; element('touch').hidden = false;
  element('look-hint').hidden = false;
}
function fail(error: unknown): void {
  // Navigation cancels pending HTTP reads. It is not a failure of the next page.
  if (leavingPage) return;
  playing = false; connected = false; clearInput();
  if (display instanceof OriginalWorldRenderer) display.setPlaying(false);
  showMenu(error instanceof Error ? error.message : String(error));
  element('retry').hidden = false;
  console.error(error);
}

async function control(action: string, fields: Record<string, unknown> = {}): Promise<void> {
  controlsPending++; busy = true; controlVersion++;
  if (['start', 'pause', 'resume', 'load', 'reset'].includes(action)) {
    lastArrival = 0; intervals.length = 0;
  }
  clearInput();
  try {
    const result = await post('/control', { action, ...fields });
    if (action === 'start' || action === 'resume') {
      started = true; playing = true; hideMenu(); canvas.focus();
      if (!polling) { polling = true; void pollFrame(); }
    }
    else if (action === 'pause') { playing = false; showMenu('Paused'); }
    else if (action === 'save') { message.textContent = 'Game saved.'; element('hint').textContent = 'Game saved.'; }
    else if (action === 'load' || action === 'reset') {
      history.reset(); playing = result.running === true; showMenu(action === 'load' ? 'Game loaded.' : 'Level restarted.');
      if (playing) hideMenu();
    }
    if (action !== 'start' && document.pointerLockElement === canvas) { intentionalUnlock = true; document.exitPointerLock(); }
    if (display instanceof OriginalWorldRenderer) display.setPlaying(playing);
  } catch (error) {
    // A refused save/selection leaves the existing native owner intact.
    message.textContent = error instanceof Error ? error.message : String(error);
    if (!playing) showMenu(message.textContent);
    else { element('hint').textContent = message.textContent; element('hint').hidden = false; }
  }
  finally { controlsPending--; busy = controlsPending > 0; }
}

let display: IndexedDisplay | OriginalWorldRenderer;
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
    const geometry = display instanceof OriginalWorldRenderer;
    const response = await fetch(API + (geometry ? '/world-frame' : '/frame'), { cache: 'no-store', signal: AbortSignal.timeout(10000) });
    if (!response.ok) { await responseJson(response); return; }
    const data = await response.arrayBuffer();
    const frame = geometry ? decodeWorldFrame(data) : decodeOriginalFrame(data);
    if (version !== controlVersion) return;
    if (lastFrame && frame.runtimeGeneration === lastFrame.runtimeGeneration && frame.sequence <= lastFrame.sequence) return;
    const query = '?sequence=' + frame.sequence;
    const [stateResponse, observationResponse] = await Promise.all([
      fetch(API + '/state' + query, { cache: 'no-store', signal: AbortSignal.timeout(10000) }),
      fetch(API + '/observation' + query, { cache: 'no-store', signal: AbortSignal.timeout(10000) }),
    ]);
    if (stateResponse.status === 410 || observationResponse.status === 410) return;
    const state = await responseJson<BridgeState>(stateResponse);
    if (!observationResponse.ok) { await responseJson(observationResponse); return; }
    const observation = decodeRenderPublication(new Uint8Array(await observationResponse.arrayBuffer()));
    if (observation.runtime_generation !== frame.runtimeGeneration || observation.identity.generation !== frame.sessionGeneration ||
        observation.identity.tick !== frame.tick || BigInt(state.sequence) !== frame.sequence ||
        BigInt(state.runtime_generation) !== frame.runtimeGeneration || BigInt(state.session_generation) !== frame.sessionGeneration || BigInt(state.tick) !== frame.tick)
      throw new Error('The original scene and HUD do not describe the same native frame');
    if (version !== controlVersion) return;
    lastFrame = frame;
    nativeMenus = state.source_menu_admission ?? { map_open: false, spells_open: false };
    document.body.dataset.gameMenu = element('touch').dataset.menu = nativeMenus.spells_open ? 'spells' : 'flight';
    element('touch-hand').hidden = !nativeMenus.spells_open;
    element('touch-spells').textContent = nativeMenus.spells_open ? 'Close spells' : 'Spells';
    const source = observation.value;
    spellMenu = { open: nativeMenus.spells_open, hovered_slot: hoveredSlot, hovered_tier: hoveredTier,
      mana: source.player_vitals.mana, wizard_colors: source.spell_menu_wizard_colors,
      panels_opaque: source.spell_menu_panels_opaque, spells: source.player_spells };
    const objectiveSelector = source.objective_secret_announcement_record ??
      (state.objective ? state.objective.is_level_end ? 53 : 48 + state.objective.text_selector : null);
    const hudState: OriginalHudState = { ...state, source_top_status: source.source_top_status,
      panels_opaque: source.spell_menu_panels_opaque, objective_selector: objectiveSelector,
      objective_visible: source.objective_textbox_ticks > 0, spell_menu: spellMenu };
    // Flight history is independent of map presentation and remains at the
    // correct native revision while the source split map is displayed.
    if (display instanceof OriginalWorldRenderer) {
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
      // Source art stays indexed. Opaque panels avoid flattening a translucent
      // HUD against an invented background; the 3D world is never read to CPU.
      hudState.panels_opaque = true;
      const overlay = hud.compose(emptyScene, frame.palette, hudState, hudCoverage);
      display.present(worldFrame, source.source_camera_pose, source.source_camera_fov, overlay, hudCoverage, nativeMenus.map_open, playing && !state.paused);
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
    display.present(hud.compose(nativeMenus.map_open ? visible.indices : scene, visible.palette, hudState), visible.palette);
    }
    Object.assign(window, { originalState: state, originalObservation: observation });
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
  finally { if (connected) window.setTimeout(() => void pollFrame(), Math.max(5, 1000 / 30 - (performance.now() - before))); }
}

async function sendInput(): Promise<void> {
  if (!playing || busy || inputBusy) return; inputBusy = true;
  const cast = queuedCasts > 0;
  const command = originalInput(nativeMenus.spells_open || nativeMenus.map_open ? new Set<string>() : keys, mouseX, mouseY,
    { selected: cast || keys.has('Space'), left: mouseButtons.has(0), right: mouseButtons.has(2) });
  mouseX *= .65; mouseY *= .65;
  try { await post('/input', { command }); if (cast) queuedCasts = Math.max(0, queuedCasts - 1); }
  catch (error) { fail(error); }
  finally { inputBusy = false; }
}

playButton.addEventListener('click', () => void control('start'));
pauseButton.addEventListener('click', () => void control('resume'));
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
  if (display instanceof OriginalWorldRenderer) return display.hudPoint(x, y);
  const rect = canvas.getBoundingClientRect(); return [(x - rect.left) * 640 / rect.width, (y - rect.top) * 480 / rect.height];
};
canvas.addEventListener('pointerdown', event => {
  if (!playing) return;
  if (nativeMenus.spells_open && spellMenu) {
    event.preventDefault();
    const hit = hud.hitTest(...hudPoint(event.clientX, event.clientY), spellMenu);
    if (hit) { hoveredSlot = hit.hoveredSlot; hoveredTier = hit.hoveredTier; }
    if (hit?.spellId != null && hit.tier != null && (event.button === 0 || event.button === 2))
      void control('assign_spell', { spell_id: hit.spellId, tier: hit.tier, hand: event.pointerType === 'touch' ? touchHand : event.button === 0 ? 'left' : 'right' });
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
window.addEventListener('pointerup', event => mouseButtons.delete(event.button));
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
  if (!started || event.target instanceof HTMLInputElement || event.target instanceof HTMLSelectElement) return;
  if (!playing && !['Escape','KeyP','F5','F9'].includes(event.code)) return;
  if (['Space','ArrowLeft','ArrowRight','ArrowUp','ArrowDown','Escape','F5','F9','Tab'].includes(event.code)) event.preventDefault();
  keys.add(event.code);
  if (event.repeat) return;
  if (event.code === 'Space') queuedCasts = Math.min(4, queuedCasts + 1);
  if (event.code === 'Escape' && (nativeMenus.spells_open || nativeMenus.map_open)) void control('menu', { menu_action: 'close_gameplay_menus' });
  else if (event.code === 'Escape' || event.code === 'KeyP') void control(playing ? 'pause' : 'resume');
  if (event.code === 'ControlLeft' || event.code === 'ControlRight') void control('menu', { menu_action: 'open_spell_menu' });
  if (event.code === 'F5') void control('save');
  if (event.code === 'F9') void control('load');
  if (event.code === 'Tab') void control('map');
});
window.addEventListener('keyup', event => {
  keys.delete(event.code);
  if (playing && (event.code === 'ControlLeft' || event.code === 'ControlRight')) void control('menu', { menu_action: 'close_spell_menu' });
});
window.addEventListener('blur', () => { clearInput(); if (playing) void control('pause'); });
window.addEventListener('beforeunload', () => { leavingPage = true; connected = false; playing = false; clearInput(); });
document.addEventListener('visibilitychange', () => { if (document.hidden) { clearInput(); if (playing) void control('pause'); } });
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
      status = await responseJson<BridgeStatus>(await fetch(API + '/status', { cache: 'no-store', signal: AbortSignal.timeout(10000) }));
      if (status.error) throw new Error(status.error);
      if (status.ready) break;
      await new Promise(resolve => window.setTimeout(resolve, 300));
    }
    if (!status?.ready) throw new Error('The original game did not finish loading.');
    if (status.renderer === 'geometry') {
      document.body.dataset.renderer = 'geometry';
      display = await OriginalWorldRenderer.load(canvas, API);
      display.onError = fail;
    } else display = new IndexedDisplay();
    hud = await OriginalHud.load(API); tables = hud.tables;
    inputSequence = status.next_sequence ?? 0; connected = true;
    playButton.disabled = false; message.textContent = 'The original world is ready.';
    window.setInterval(() => void sendInput(), 1000 / 30);
  } catch (error) { fail(error); }
}
void boot();
