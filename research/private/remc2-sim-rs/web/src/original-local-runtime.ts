import { decodeRenderPublication } from './render-publication';
import { OriginalWasi, parseLayout, type Layout, type MissionInputs } from './original-wasi';

export type LocalReply = { status?: number; body?: ArrayBuffer | string; headers?: Record<string, string> };
type Command = Record<string, unknown>;
type MenuAdmission = { action: string; admitted: boolean; map_open: boolean; spells_open: boolean; player_information_open: boolean };
type Identity = {
  generation: number | string;
  mode: string;
  session: { generation: number | string; tick: number | string; session_id: string } | null;
  active_runtime_flow?: { bound_level_id: string };
  player_preferences?: { preferences?: { keys?: string[] } };
  campaign_menu?: { phase: string };
  display_settings?: { reflections: boolean };
};
const neutral = { longitudinal: 'coast', lateral: 'neutral', roll_axis: 0, pitch_axis: 0,
  cast_selected: false, cast_left: false, cast_right: false };
const edges = ['cast_selected', 'cast_left', 'cast_right', 'recover_player', 'full_stop', 'surrender_player', 'remove_castle_stage'];
const encoder = new TextEncoder();
const json = (body: unknown, status = 200): LocalReply => ({ status, body: JSON.stringify(body), headers: { 'Content-Type': 'application/json' } });

class TerrainPublication {
  revision = 0;
  session = 0n;
  bytes: Uint8Array<ArrayBuffer> = new Uint8Array(0);

  refresh(core: OriginalWasi, handle: bigint, generation: bigint): void {
    const planeSize = 65_536, length = 5 * planeSize;
    const scratch = core.scratch(length + 8);
    for (let kind = 0; kind < 5; kind++) {
      core.call('mc_native_campaign_terrain_plane_copy_u8', handle, generation, kind,
        scratch + 8 + kind * planeSize, planeSize, scratch);
      if (core.view().getUint32(scratch, true) !== planeSize) throw new Error('Incomplete WASM terrain plane');
    }
    const next = core.bytes(scratch + 8, length);
    if (generation !== this.session || next.length !== this.bytes.length || next.some((value, at) => value !== this.bytes[at])) {
      this.bytes = next.slice();
      this.session = generation;
      this.revision++;
    }
  }
}

export class OriginalLocalRuntime {
  private identity!: Identity;
  private handle: bigint;
  private input: Command = { ...neutral };
  private controls: Command[] = [];
  private running = false;
  private drawReady = false;
  private deadline = 0;
  private clockRemainder = 0;
  private sequence = 0;
  private nextRequest = 0;
  private lastRequest?: { body: string; reply: LocalReply };
  private packet: ArrayBuffer | null = null;
  private lastMenu: MenuAdmission | null = null;
  private currentLife = 0;
  private failure: string | null = null;
  private readonly terrain = new TerrainPublication();

  constructor(private readonly core: OriginalWasi, private readonly inputs: MissionInputs,
    private readonly textureSizes: Map<number, number>) {
    const scratch = core.scratch(8);
    core.call('mc_game_runtime_create_v1', scratch);
    this.handle = core.view().getBigUint64(scratch, true);
    this.refreshIdentity();
    core.send('mc_game_runtime_bootstrap_package_set_json_v1', [this.handle, this.outer], {
      request_contract: 'carpet-flight-installed-package-set-bootstrap/v1', packages: inputs.packages,
    });
    this.refreshIdentity();
    core.call('mc_game_runtime_open_level_v1', this.handle, this.outer);
    this.refreshIdentity();
    core.call('mc_game_runtime_pause_v1', this.handle, this.outer);
    this.refreshIdentity();
    core.send('mc_game_runtime_preferences_command_json_v1', [this.handle, this.outer], {
      operation: 'install_languages', cache_dir: '/language',
    });
    this.refreshIdentity();
    core.call('mc_game_runtime_resume_v1', this.handle, this.outer);
    this.refreshIdentity();
    this.publish();
    setInterval(() => this.pump(), 5);
  }

  private refreshIdentity(): void {
    this.identity = this.core.json('mc_game_runtime_identity_json_v1', [this.handle], 65_536);
  }
  private get outer(): bigint { return BigInt(this.identity.generation); }
  private get generation(): bigint { return BigInt(this.identity.session!.generation); }
  private get tick(): bigint { return BigInt(this.identity.session!.tick); }
  private get paused(): boolean { return this.identity.mode === 'paused'; }
  private get native(): [bigint, bigint, bigint] { return [this.handle, this.generation, this.tick]; }
  private get guarded(): [bigint, bigint, bigint, bigint] { return [this.handle, this.outer, this.generation, this.tick]; }

  private status() {
    return { ready: this.failure === null, error: this.failure, running: this.running, paused: this.paused,
      next_sequence: this.nextRequest, asset_revision: 1, level_id: this.identity.active_runtime_flow?.bound_level_id,
      phase: this.identity.campaign_menu?.phase ?? this.identity.mode, campaign_menu: this.identity.campaign_menu,
      renderer: 'geometry', preference_keys: this.identity.player_preferences?.preferences?.keys ?? [],
      speech_available: false, has_save: false };
  }

  private snapshot(name: string, args: (number | bigint)[], layout: Layout) {
    const offset = this.core.scratch(layout.size);
    this.core.call(name, ...args, offset, layout.size);
    return this.checkedLayout(offset, layout);
  }
  private checkedLayout(offset: number, layout: Layout) {
    const row = parseLayout(this.core.view(), offset, layout);
    if (row.abi_version !== 2 || row.struct_size !== layout.size || row.generation !== this.generation.toString() ||
        (row.tick !== undefined && row.tick !== this.tick.toString())) throw new Error('Stale WASM presentation record');
    return row;
  }
  private mapRows(offset: number, count: number, layout: Layout) {
    return Array.from({ length: count }, (_, index) => this.checkedLayout(offset + index * layout.size, layout));
  }
  private minimap() {
    const { MinimapState, MinimapEntity, MinimapMark } = this.inputs.layouts;
    const state = this.snapshot('mc_native_campaign_minimap_state', this.native, MinimapState);
    const rows = (name: string, count: number, layout: Layout) => {
      if (!Number.isInteger(count) || count < 0 || count > 4096) throw new Error('Invalid WASM minimap row count');
      if (!count) return [];
      const scratch = this.core.scratch(count * layout.size + 8);
      this.core.call(name, ...this.native, 0, scratch + 8, count, layout.size, scratch);
      if (this.core.view().getUint32(scratch, true) !== count) throw new Error('Incomplete WASM minimap');
      return this.mapRows(scratch + 8, count, layout);
    };
    const entities = rows('mc_native_campaign_minimap_entities_copy', state.entity_count, MinimapEntity);
    const marks = rows('mc_native_campaign_minimap_marks_copy', state.mark_count, MinimapMark);
    const coverage = this.core.read('mc_native_campaign_minimap_terrain_coverage_copy', this.native, 4096);
    if (coverage.length !== 4096) throw new Error('Incomplete WASM minimap coverage');
    const labels = state.visibility_flags & 8 ? this.core.json('mc_native_campaign_minimap_labels_json', this.native, 8192) : [];
    return { entities, marks, coverage: Array.from(coverage), visibility_flags: state.visibility_flags, labels };
  }
  private worldMap() {
    const { MinimapState, MinimapEntity, MinimapMark } = this.inputs.layouts;
    const entityLimit = 1000, markLimit = 64;
    const sceneBytes = 64_000, paletteBytes = 768;
    const base = this.core.scratch(sceneBytes + paletteBytes + entityLimit * MinimapEntity.size + markLimit * MinimapMark.size + MinimapState.size);
    const palette = base + sceneBytes;
    const entities = palette + paletteBytes;
    const marks = entities + entityLimit * MinimapEntity.size;
    const state = marks + markLimit * MinimapMark.size;
    this.core.call('mc_native_campaign_world_map_copy', ...this.native, base, sceneBytes, palette, paletteBytes,
      entities, entityLimit, marks, markLimit, state, MinimapState.size);
    const row = this.checkedLayout(state, MinimapState);
    if (row.entity_count > entityLimit || row.mark_count > markLimit) throw new Error('Invalid WASM world map extents');
    const result = { entities: this.mapRows(entities, row.entity_count, MinimapEntity),
      marks: this.mapRows(marks, row.mark_count, MinimapMark), visibility_flags: row.visibility_flags };
    const labels = row.visibility_flags & 8 ? this.core.json('mc_native_campaign_world_map_labels_json', this.native, 8192) : [];
    return { ...result, labels };
  }

  private tmapPixels() {
    const bytes = this.core.read('mc_game_runtime_tmap_pixels_copy_v1', this.guarded, 2 * 1024 * 1024);
    const view = new DataView(bytes.buffer);
    const count = view.getUint16(0, true);
    if (count > 504) throw new Error('Invalid WASM TMAP count');
    let at = 2, lastSelector = -1;
    const images: { selector: number; pixels: string }[] = [];
    for (let i = 0; i < count; i++) {
      const selector = view.getUint16(at, true), length = view.getUint32(at + 2, true);
      at += 6;
      if (selector <= lastSelector || selector >= 504 || !length || at + length > bytes.length)
        throw new Error('Invalid WASM TMAP extent');
      const expected = this.textureSizes.get(selector);
      if (expected !== undefined) {
        if (length !== expected) throw new Error('WASM TMAP does not match the atlas');
        const pixels = bytes.subarray(at, at + length);
        const text: string[] = [];
        for (let j = 0; j < pixels.length; j += 8192) text.push(String.fromCharCode(...pixels.subarray(j, j + 8192)));
        images.push({ selector, pixels: btoa(text.join('')) });
      }
      at += length;
      lastSelector = selector;
    }
    if (at !== bytes.length) throw new Error('Trailing WASM TMAP data');
    return images;
  }

  private publish(): void {
    if (!this.identity.session) { this.packet = null; return; }
    const core = this.core;
    core.call('mc_game_runtime_presentation_frame_v1', ...this.guarded, 1);
    const observation = core.read('mc_game_runtime_render_observation_copy_v1', this.guarded, 2 * 1024 * 1024);
    const { value: observed } = decodeRenderPublication(observation);
    const camera = observed.source_camera_pose;
    const palette = core.read('mc_native_campaign_palette_copy', this.native, 768);
    if (palette.length !== 768) throw new Error('Invalid WASM palette');
    this.terrain.refresh(core, this.handle, this.generation);
    const scratch = core.scratch(4096 * 64 + 8);
    core.call('mc_native_campaign_world_entity_presentations_copy', ...this.native, camera.yaw, scratch + 8, 4096, 64, scratch);
    const rowCount = core.view().getUint32(scratch, true);
    if (rowCount > 4096) throw new Error('Invalid WASM world row count');
    const frame = new Uint8Array(832 + rowCount * 64);
    const view = new DataView(frame.buffer);
    frame.set(encoder.encode('MCW1'));
    view.setUint16(4, 1, true); view.setUint16(6, 64, true);
    view.setBigUint64(8, this.outer, true); view.setBigUint64(16, this.generation, true);
    view.setBigUint64(24, this.tick, true); view.setBigUint64(32, BigInt(++this.sequence), true);
    view.setBigUint64(40, BigInt(this.terrain.revision), true);
    view.setUint32(48, rowCount, true); view.setUint32(52, 64, true);
    frame.set(palette, 64);
    frame.set(core.bytes(scratch + 8, rowCount * 64), 832);
    const hud = this.snapshot('mc_native_campaign_day_hud_state', this.native, this.inputs.layouts.Hud);
    const objective = this.snapshot('mc_native_campaign_objective_stage_state', [this.handle, this.generation, 0, 0], this.inputs.layouts.Objective);
    const menuOffset = core.scratch(8);
    core.call('mc_game_runtime_player_menu_state_v1', ...this.guarded, menuOffset);
    const menuState = core.view().getUint8(menuOffset);
    const menu = observed.source_menu_admission;
    if (menu) this.lastMenu = { action: menu.action, admitted: menu.admitted, map_open: menu.map_open,
      spells_open: menu.spells_open, player_information_open: menu.player_information_open };
    this.currentLife = observed.player_vitals.life;
    const preferences = this.identity.player_preferences?.preferences;
    const state = encoder.encode(JSON.stringify({ sequence: this.sequence, runtime_generation: this.outer.toString(),
      session_generation: this.generation.toString(), tick: this.tick.toString(), asset_revision: 1,
      paused: this.paused, player_pose: observed.player_pose, source_menu_admission: this.lastMenu, source_menu_state: menuState,
      hud, minimap: this.minimap(), objective, tmap_pixels: this.tmapPixels(),
      ...(this.lastMenu?.map_open ? { world_map: this.worldMap() } : {}),
      preference_keys: preferences?.keys ?? [], map_key: preferences?.keys?.[4] ?? null,
      reflections: this.identity.display_settings?.reflections === true, speech_available: false,
    }));
    const packet = new Uint8Array(16 + frame.length + state.length + observation.length);
    const header = new DataView(packet.buffer);
    packet.set(encoder.encode('MCB1'));
    header.setUint32(4, frame.length, true); header.setUint32(8, state.length, true); header.setUint32(12, observation.length, true);
    packet.set(frame, 16); packet.set(state, 16 + frame.length); packet.set(observation, 16 + frame.length + state.length);
    this.packet = packet.buffer;
  }

  private pump(): void {
    if (!this.running || !this.drawReady || performance.now() < this.deadline) return;
    try {
      const command = this.controls.shift() ?? { ...this.input };
      if (command.full_stop) Object.assign(command, { cast_selected: false, recover_player: false, remove_castle_stage: false });
      else if (command.cast_selected || command.recover_player) {
        if (this.currentLife < 0) Object.assign(command, { cast_selected: false, recover_player: true });
        else if (command.recover_player) command.recover_player = false;
      }
      this.clockRemainder += 100;
      command.elapsed_centiseconds = Math.floor(this.clockRemainder / 30);
      this.clockRemainder %= 30;
      this.core.send('mc_game_session_step_json_v1', this.native, command);
      this.refreshIdentity();
      for (const edge of edges) this.input[edge] = false;
      this.drawReady = false;
      this.deadline = Math.max(this.deadline + 1000 / 30, performance.now());
      const changed = this.core.scratch(8);
      this.core.call('mc_game_runtime_finish_mission_if_ready_result_v1', ...this.guarded, changed);
      if (this.core.view().getUint8(changed)) { this.running = false; this.refreshIdentity(); }
      this.publish();
    } catch (error) {
      this.failure = error instanceof Error ? error.message : String(error);
      this.running = false;
      this.packet = null;
    }
  }

  private control(value: Command): void {
    switch (value.action) {
      case 'start': case 'resume':
        if (this.paused) this.core.call('mc_game_runtime_resume_v1', this.handle, this.outer);
        this.running = true;
        this.deadline = performance.now();
        break;
      case 'pause':
        if (!this.paused) this.core.call('mc_game_runtime_pause_v1', this.handle, this.outer);
        this.running = false;
        this.input = { ...neutral }; this.controls = [];
        break;
      case 'reset':
        if (this.paused) {
          this.core.call('mc_game_runtime_resume_v1', this.handle, this.outer);
          this.refreshIdentity();
        }
        this.core.call('mc_game_session_reset_v1', ...this.native);
        this.refreshIdentity();
        this.core.call('mc_game_runtime_pause_v1', this.handle, this.outer);
        this.running = false; this.drawReady = false;
        this.input = { ...neutral }; this.controls = []; this.clockRemainder = 0; this.lastMenu = null;
        break;
      case 'map': this.queueTurn({ abandon_action: 'toggle_gameplay_map' }); return;
      case 'menu': this.queueTurn({ abandon_action: value.menu_action }); return;
      case 'select_spell': this.queueTurn({ select_spell: value.spell_id }); return;
      case 'assign_spell': this.queueTurn({ assign_spell: { spell_id: value.spell_id, tier: value.tier, hand: value.hand } }); return;
      case 'mark_spell': this.queueTurn({ mark_spell: { spell_id: value.spell_id, hand: value.hand, holder_actor: value.holder_actor } }); return;
      case 'cycle_marked_spell': this.queueTurn({ cycle_marked_spell: { hand: value.hand, direction: value.direction } }); return;
      case 'tmap_draw': {
        if (!this.identity.session || value.runtime_generation !== this.outer.toString() ||
            value.session_generation !== this.generation.toString() || value.tick !== this.tick.toString()) return;
        const selectors = value.selectors;
        if (!Array.isArray(selectors) || selectors.length > 504 || !selectors.every((n, i) =>
          Number.isInteger(n) && n >= 0 && n < 504 && (i === 0 || n > selectors[i - 1]))) throw new Error('Invalid WASM draw selectors');
        const scratch = this.core.scratch(Math.max(8, selectors.length * 2));
        selectors.forEach((n, i) => this.core.view().setUint16(scratch + i * 2, n, true));
        this.core.call('mc_game_runtime_tmap_draw_v1', ...this.guarded, Number(value.yaw), scratch, selectors.length);
        this.drawReady = true;
        return;
      }
      default: throw new Error('This control is unavailable in the one-mission WASM playtest.');
    }
    this.refreshIdentity();
    this.publish();
  }
  private queueTurn(fields: Command): void {
    if (!this.running || this.paused || this.controls.length >= 128) throw new Error('The mission is not accepting turn controls');
    this.controls.push({ ...neutral, ...fields });
    if (fields.abandon_action === 'toggle_gameplay_map') for (const edge of edges) this.input[edge] = false;
  }

  request(path: string, body?: string): LocalReply {
    if (path === '/status') return json(this.status());
    if (this.failure) return json({ error: this.failure }, 500);
    const [endpoint, query] = path.split('?', 2);
    const params = new URLSearchParams(query);
    if (endpoint === '/world-packet') {
      if (!this.packet) return { status: 204 };
      if (params.get('after') === String(this.sequence) && params.get('generation') === this.outer.toString())
        return { status: 204, headers: { 'X-Scene-Pending': '1' } };
      return { body: this.packet.slice(0) };
    }
    if (endpoint === '/terrain') return { body: this.terrain.bytes.slice().buffer };
    if (!body) return json({ error: 'Invalid local game request' }, 400);
    if (body === this.lastRequest?.body) return this.lastRequest.reply;
    const value = JSON.parse(body) as Command;
    if (value.sequence !== this.nextRequest) return json({ error: 'Local game command out of sequence' }, 409);
    if (endpoint === '/input') {
      const command = { ...value.command as Command };
      for (const edge of edges) if (this.input[edge]) command[edge] = true;
      this.input = command;
    } else if (endpoint === '/control') this.control(value);
    else return json({ error: 'Unknown local game request' }, 404);
    this.nextRequest++;
    const reply = json(this.status());
    this.lastRequest = { body, reply };
    return reply;
  }
}
