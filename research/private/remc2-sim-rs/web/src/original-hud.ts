/** Indexed Original HUD. All artwork and dynamic selectors come from the local owner cache / Rust. */
import type { IndexedBlendTables } from './original-adapter';

type Span = { offset: number; length: number };
type SpriteRow = { selector: number; width: number; height: number; runtime_width: number; runtime_height: number; indices: Span; mask: Span };
type ObjectiveRow = { selector: number; reference_x: number; reference_y: number; render_width: number; render_height: number; blend_x: number; blend_y: number; blend_width: number; blend_height: number; indices: Span; mask: Span };
type TierRow = { tier: number; width: number; height: number; color: number; rgba: Span };
type SymbolRow = { selector: number; width: number; height: number; indices: Span; mask: Span };
type HelpRow = { text: string; text_record: number; flags: number; duration: number; age_threshold: number; priority: number; width: number; height: number; box_width?: number; mask: Span; advances: number[]; color: number; background: number; line_color: number; frame: SymbolRow[] };
type FontGlyph = { selector: number; width: number; height: number; mask: Span };
type Manifest = { schema: string; bytes: number; normal: Span; exit: Span; aperture: Span; objective_background: number; map_cross_color: number; sprites: SpriteRow[]; symbols: SymbolRow[]; objectives: ObjectiveRow[]; objective_stage_records: number[]; objective_end_record: number; tiers: TierRow[]; font1?: FontGlyph[]; gameplay_font?: FontGlyph[]; help?: Record<string, HelpRow> | null };

export interface OriginalHudSlot {
  holder_entity_index: number; draw_admitted: number; frame_selector: number; icon_selector: number;
  level_text_selector: number; max_mana: number; caster_mana: number;
}
export interface OriginalHudSnapshot {
  generation: number | string; tick: number | string;
  minimap_panel_selector: number; castle_panel_selector: number; balloon_panel_selector: number;
  player_panel_selector: number; player_icon_selector: number; life_mana_icon_selector: number;
  local_player_life: number; local_player_max_life: number; local_player_mana: number;
  local_player_max_mana: number; global_mana_denominator: number;
  color_phase_2: number;
  life_palette_index: number; mana_palette_index: number; mana_max_palette_index: number;
  left: OriginalHudSlot; right: OriginalHudSlot;
}
export interface OriginalHudState {
  hud?: OriginalHudSnapshot | null;
  objective_selector?: number | null;
  objective_visible?: boolean;
  panels_opaque?: boolean;
  top_bar_visible?: boolean;
  minimap?: (MapCommands & { terrain: number[]; coverage: number[] }) | null;
  world_map?: MapCommands | null;
  exit_inspection_allowed?: boolean;
  exit_inspection_subject?: number | null;
  source_top_status?: SourceTopStatus | null;
  spell_menu?: SpellMenu | null;
  player_information?: { players: readonly { source_slot: number; display_name: string; outline_palette_index: number; text_palette_index: number; max_mana: number; kills8: readonly number[] }[]; projection_error: string | null } | null;
  pause_menu?: { open: boolean; can_load: boolean; sound_enabled: boolean; music_enabled: boolean } | null;
}
export interface SpellMenuRow {
  menu_slot: number; unlocked_tier: number; marked_hand: string;
  id: string; acquired: boolean; holder_actor?: bigint | string;
  menu_artwork?: { grid_icon: number; tier_icons: readonly number[] };
  menu_help?: { is_standard: boolean; selected_tier: number; castle_blocked: readonly boolean[]; definition_enabled: boolean;
    blink_hidden: boolean; discovered: boolean; tier_mana_costs: readonly number[]; experience: number; experience_thresholds: readonly number[] };
}
export interface SpellMenu { open: boolean; hovered_slot: number; hovered_tier: number; mana: number; wizard_colors: readonly number[]; panels_opaque: boolean; spells: readonly SpellMenuRow[] }
interface SourceCastleStatus { stage: number; panel_selector: number; icon_selector: number; life: number; max_life: number; mana: number; max_mana: number; owner_mana_aggregate: number }
interface SourceTopStatus { castle_rows: readonly SourceCastleStatus[]; balloon_panel_selector: number; balloon_icon_selector: number; balloons: readonly { present: boolean; life: number; max_life: number; mana: number; max_mana: number }[]; mana_goal_percent: number; goal_palette_index: number }
type MapLabel = { source_order: number; subject_entity_index: number; x: number; y: number; palette_index: number; text: string };
type MapCommands = { entities: MinimapEntity[]; marks: MinimapMark[]; labels?: MapLabel[] };
type MinimapEntity = { generation: number | string; tick: number | string; source_order?: number; x: number; y: number; selector: number; palette_index: number; kind: number; size: number; anchor: number; visible: number; subject_entity_index?: number; help_selector?: number };
type MinimapMark = { generation: number | string; tick: number | string; source_order: number; x: number; y: number; palette_index: number; kind: number; visible: number };
export interface MapHintContext {
  sequence: number | string; generation: number | string; enabled: boolean; paused: boolean; suppressed: boolean;
  player: { x: number; y: number }; entities: readonly { entity: number; x: number; y: number }[];
}

export class OriginalHud {
  readonly width = 640;
  readonly height = 480;
  readonly tables: IndexedBlendTables;
  private readonly sprites = new Map<number, SpriteRow>();
  private readonly symbols = new Map<number, SymbolRow>();
  private readonly mapFont = new Map<number, FontGlyph>();
  private readonly gameplayFont = new Map<number, FontGlyph>();
  private readonly objectives = new Map<number, ObjectiveRow>();
  private readonly objectiveStages: number[];
  private readonly objectiveEnd: number;
  private readonly tiers = new Map<number, TierRow>();
  private readonly exitHelp?: HelpRow;
  private readonly mapHelp = new Map<number, HelpRow>();
  private hintRows = new Map<number, { age: number; priority: number }>();
  private hintGeneration = '';
  private hintSequence = '';
  private hintEnabled = false;
  private hintDelay = 0;
  private hintRemaining = 0;
  private hintCurrent: { row: number; subject: number; x: number; y: number } | null = null;
  private hintDraw: { row: number; subject: number; x: number; y: number } | null = null;
  private readonly objectiveBackground: number;
  private readonly mapCrossColor: number;
  private readonly aperture: Uint8Array;
  private readonly blob: Uint8Array;
  private constructor(blob: Uint8Array, manifest: Manifest) {
    this.blob = blob;
    if (manifest.schema !== 'mc-original-assets/v1' || manifest.bytes !== blob.length) throw new Error('Invalid Original asset manifest');
    const span = (item: Span) => {
      if (!Number.isSafeInteger(item.offset) || !Number.isSafeInteger(item.length) || item.offset < 0 || item.length < 0 || item.offset + item.length > blob.length) throw new Error('Invalid Original asset span');
      return blob.subarray(item.offset, item.offset + item.length);
    };
    this.tables = { normal: span(manifest.normal), exit: span(manifest.exit) };
    const exitHelp = manifest.help?.['83'];
    if (exitHelp) {
      if (exitHelp.text_record !== 345 || exitHelp.flags !== 0x280 || exitHelp.duration !== 50 ||
          exitHelp.age_threshold !== 100 || exitHelp.priority !== 5 || !exitHelp.text ||
          !Number.isInteger(exitHelp.width) || exitHelp.width <= 0 || !Number.isInteger(exitHelp.height) || exitHelp.height <= 0 ||
          !Number.isInteger(exitHelp.color) || exitHelp.color < 0 || exitHelp.color > 255 ||
          !Number.isInteger(exitHelp.background) || exitHelp.background < 0 || exitHelp.background > 255 ||
          !Number.isInteger(exitHelp.line_color) || exitHelp.line_color < 0 || exitHelp.line_color > 255 ||
          span(exitHelp.mask).length !== exitHelp.width * exitHelp.height ||
          !Array.isArray(exitHelp.advances) || exitHelp.advances.length !== exitHelp.text.length ||
          !Array.isArray(exitHelp.frame) || exitHelp.frame.length !== 3 ||
          exitHelp.frame.some((row, index) => row.selector !== 171 + index || row.width <= 0 || row.height <= 0 ||
            span(row.indices).length !== row.width * row.height || span(row.mask).length !== row.width * row.height))
        throw new Error('Invalid Original exit help asset');
      this.exitHelp = exitHelp;
    }
    for (const [id, record, flags] of [[19, 307, 0x400], [20, 308, 0]] as const) {
      const help = manifest.help?.[String(id)];
      if (!help) continue;
      if (help.text_record !== record || help.flags !== flags || help.duration !== 50 || help.age_threshold !== 160 ||
          help.priority !== 5 || !help.text || !Number.isInteger(help.box_width) || help.box_width! < help.width ||
          help.box_width! > 220 || help.width <= 0 || help.height <= 0 || help.height > 64 ||
          span(help.mask).length !== help.width * help.height || help.frame.length !== 3 ||
          help.frame.some((row, index) => row.selector !== 171 + index || row.width <= 0 || row.height <= 0 ||
            span(row.indices).length !== row.width * row.height || span(row.mask).length !== row.width * row.height))
        throw new Error('Invalid Original creature or pickup help asset');
      this.mapHelp.set(id, help);
      this.hintRows.set(id, { age: help.age_threshold, priority: help.priority });
    }
    if (!Number.isInteger(manifest.objective_background) || manifest.objective_background < 0 || manifest.objective_background > 255) throw new Error('Invalid Original objective color');
    this.objectiveBackground = manifest.objective_background;
    if (!Number.isInteger(manifest.map_cross_color) || manifest.map_cross_color < 0 || manifest.map_cross_color > 255) throw new Error('Invalid Original map cross color');
    this.mapCrossColor = manifest.map_cross_color;
    this.aperture = span(manifest.aperture);
    if (this.aperture.length !== 256) throw new Error('Invalid Original minimap aperture');
    if (this.tables.normal.length !== 65536 || this.tables.exit.length !== 65536) throw new Error('Invalid Original blend table');
    for (const row of manifest.sprites) {
      if (row.width <= 0 || row.height <= 0 || row.runtime_width <= 0 || row.runtime_height <= 0 || span(row.indices).length !== row.width * row.height || span(row.mask).length !== row.width * row.height || this.sprites.has(row.selector)) throw new Error('Invalid Original sprite');
      this.sprites.set(row.selector, row);
    }
    for (const row of manifest.symbols) {
      if (row.width <= 0 || row.height <= 0 || span(row.indices).length !== row.width * row.height || span(row.mask).length !== row.width * row.height || this.symbols.has(row.selector)) throw new Error('Invalid Original minimap symbol');
      this.symbols.set(row.selector, row);
    }
    for (const glyph of manifest.font1 ?? []) {
      if (glyph.width <= 0 || glyph.height <= 0 || span(glyph.mask).length !== glyph.width * glyph.height || this.mapFont.has(glyph.selector))
        throw new Error('Invalid Original FONT1 glyph');
      this.mapFont.set(glyph.selector, glyph);
    }
    for (const glyph of manifest.gameplay_font ?? []) {
      if (glyph.width < 0 || glyph.height < 0 || span(glyph.mask).length !== glyph.width * glyph.height || this.gameplayFont.has(glyph.selector))
        throw new Error('Invalid Original gameplay glyph');
      this.gameplayFont.set(glyph.selector, glyph);
    }
    for (const row of manifest.objectives) {
      if (span(row.indices).length !== row.render_width * row.render_height || span(row.mask).length !== row.render_width * row.render_height) throw new Error('Invalid Original objective');
      this.objectives.set(row.selector, row);
    }
    if (!Array.isArray(manifest.objective_stage_records) || manifest.objective_stage_records.length !== 8 ||
        !manifest.objective_stage_records.every(value => Number.isSafeInteger(value) && this.objectives.has(value)) ||
        !Number.isSafeInteger(manifest.objective_end_record) || !this.objectives.has(manifest.objective_end_record))
      throw new Error('Invalid Original objective record binding');
    this.objectiveStages = manifest.objective_stage_records.slice();
    this.objectiveEnd = manifest.objective_end_record;
    for (const row of manifest.tiers) {
      if (span(row.rgba).length !== row.width * row.height * 4) throw new Error('Invalid Original tier');
      this.tiers.set(row.tier, row);
    }
  }
  objectiveRecord(textSelector: number, isLevelEnd: boolean): number | null {
    return isLevelEnd ? this.objectiveEnd : this.objectiveStages[textSelector] ?? null;
  }
  static async load(base = '/original-api'): Promise<OriginalHud> {
    const [manifestResponse, blobResponse] = await Promise.all([
      fetch(`${base}/assets/manifest`, { signal: AbortSignal.timeout(15000) }),
      fetch(`${base}/assets/blob`, { signal: AbortSignal.timeout(15000) }),
    ]);
    if (!manifestResponse.ok || !blobResponse.ok) throw new Error('Original owner HUD assets unavailable');
    return new OriginalHud(new Uint8Array(await blobResponse.arrayBuffer()), await manifestResponse.json() as Manifest);
  }

  /** Deliberate map inspection; this is not the source's automatic help scan. */
  exitMarker(state: OriginalHudState, subject: number): MinimapEntity | undefined {
    if (!this.exitHelp || !state.exit_inspection_allowed || !state.world_map || !state.hud || !subject) return undefined;
    return state.world_map.entities.find(row => row.help_selector === 83 && row.subject_entity_index === subject &&
      row.visible === 1 && row.kind === 2 && (row.selector === 83 || row.selector === 84) &&
      row.x >= 0 && row.x < 191 && row.y >= 0 && row.y < 200 &&
      String(row.generation) === String(state.hud!.generation) && String(row.tick) === String(state.hud!.tick));
  }

  exitMarkerAt(state: OriginalHudState, x: number, y: number): number | null {
    if (!Number.isFinite(x) || !Number.isFinite(y) || !state.world_map) return null;
    let best: number | null = null, distance = Infinity;
    for (const row of state.world_map.entities) {
      const subject = row.subject_entity_index ?? 0;
      if (this.exitMarker(state, subject) !== row) continue;
      // compose() expands the 320×200 source map to 640×480.
      const dx = x - 2 * row.x, dy = y - row.y * 480 / 200;
      const d = dx * dx + dy * dy;
      if (Math.abs(dx) <= 16 && Math.abs(dy) <= 16 && d < distance) { best = subject; distance = d; }
    }
    return best;
  }

  exitInspectionText(state: OriginalHudState, subject: number): string | null {
    return this.exitMarker(state, subject) ? this.exitHelp!.text : null;
  }

  /** Advance once per accepted native publication, never from repaint/compose. */
  advanceMapHints(state: OriginalHudState, context: MapHintContext): void {
    const generation = String(context.generation), sequence = String(context.sequence);
    if (generation !== this.hintGeneration || (context.enabled && !this.hintEnabled)) {
      this.hintRows = new Map([...this.mapHelp].map(([row, help]) => [row, { age: help.age_threshold, priority: help.priority }]));
      this.hintCurrent = null; this.hintDelay = 0; this.hintRemaining = 0; this.hintSequence = '';
      this.hintGeneration = generation;
    }
    this.hintEnabled = context.enabled;
    if (sequence === this.hintSequence) return;
    this.hintSequence = sequence; this.hintDraw = null;
    if (!context.enabled || context.paused) return;
    const close = () => {
      if (this.hintCurrent) {
        const row = this.hintRows.get(this.hintCurrent.row)!;
        row.age = 0; row.priority = this.mapHelp.get(this.hintCurrent.row)!.priority;
      }
      this.hintCurrent = null; this.hintRemaining = 0; this.hintDelay = 20;
    };
    if (context.suppressed) { if (this.hintCurrent) close(); this.hintDelay = 20; return; }
    const map = state.world_map ?? state.minimap, full = !!state.world_map, hud = state.hud;
    const marker = (row: MinimapEntity) => {
      if (!hud || row.visible !== 1 || !row.subject_entity_index || !this.mapHelp.has(row.help_selector ?? 0) ||
          String(row.generation) !== String(hud.generation) || String(row.tick) !== String(hud.tick)) return null;
      const scale = full ? 1 : 2, x = Math.trunc(row.x / scale), y = Math.trunc(row.y / scale);
      if (row.x < 0 || row.y < 0 || x >= (full ? 191 : 64) || y >= (full ? 200 : 64)) return null;
      if (!full) {
        const phase = y * 2 < 64 ? 63 - y * 2 : y * 2 - 64;
        const half = (64 * this.aperture[phase * 4]!) >> 8;
        if (x * 2 < 64 - half || x * 2 >= 64 + half) return null;
      }
      return { row: row.help_selector!, subject: row.subject_entity_index, x: x * 2, y: y * 2 };
    };
    const marks = (map?.entities ?? []).map(marker).filter((row): row is NonNullable<typeof row> => row !== null);
    if (this.hintCurrent && this.hintRemaining) {
      const found = marks.find(row => row.row === this.hintCurrent!.row && row.subject === this.hintCurrent!.subject);
      if (!found) close();
      else { this.hintCurrent = found; this.hintDraw = found; if (--this.hintRemaining === 0) close(); }
    } else if (--this.hintDelay <= 0) {
      let best: { row: number; subject: number; x: number; y: number; overdue: number; priority: number } | null = null;
      for (const id of [19, 20]) {
        const info = this.hintRows.get(id), help = this.mapHelp.get(id);
        if (!info || !help || info.age <= help.age_threshold) continue;
        const candidates = marks.filter(row => row.row === id);
        // A missing pose must never change which subject is called nearest.
        if (candidates.some(row => !context.entities.some(entity => entity.entity === row.subject))) continue;
        let nearest: typeof candidates[number] | null = null, distance = Infinity;
        for (const candidate of candidates) {
          const entity = context.entities.find(row => row.entity === candidate.subject)!;
          const dx = ((entity.x - context.player.x + 32768) & 65535) - 32768;
          const dy = ((entity.y - context.player.y + 32768) & 65535) - 32768;
          const d = Math.floor(Math.sqrt(dx * dx + dy * dy));
          if (d < distance) { nearest = candidate; distance = d; }
        }
        const overdue = info.age - help.age_threshold;
        if (nearest && (!best || info.priority > best.priority || (info.priority === best.priority && overdue > best.overdue)))
          best = { ...nearest, overdue, priority: info.priority };
      }
      if (best) { this.hintCurrent = best; this.hintRemaining = this.mapHelp.get(best.row)!.duration; }
    }
    for (const [id, row] of this.hintRows) {
      const threshold = this.mapHelp.get(id)!.age_threshold;
      row.age++;
      if (row.age > threshold * 3) { row.age = threshold; row.priority = Math.max(0, Math.min(5, row.priority + 1)); }
    }
  }

  visibleMapHint(): Readonly<{ row: number; subject: number; x: number; y: number }> | null {
    return this.hintDraw;
  }

  /** Source logical coordinates. Returned spell is a UI intent, not an admitted action. */
  hitTest(x: number, y: number, menu: SpellMenu): { hoveredSlot: number; hoveredTier: number; spellId: string | null; tier: number | null } | null {
    if (!menu.open || !Number.isFinite(x) || !Number.isFinite(y) || x < 0 || x >= 640 || y < 0 || y >= 480) return null;
    let slot = Math.max(0, Math.min(25, menu.hovered_slot));
    let tier = Math.max(0, Math.min(2, menu.hovered_tier));
    if (y >= 408) {
      slot = Math.max(0, Math.min(1, Math.floor((y - 408) / 36))) * 13 + Math.max(0, Math.min(12, Math.floor((x - 8) / 48)));
      tier = menu.spells.find(row => row.menu_slot === slot)?.menu_help?.selected_tier ?? 0;
    } else {
      const panel = this.sprites.get(163);
      if (!panel) throw new Error('Missing Original spell tier panel');
      const start = Math.max(0, Math.min(640 - 3 * panel.runtime_width, 8 + slot % 13 * 48 + 24 - Math.trunc(3 * panel.runtime_width / 2)));
      if (y < 408 - panel.runtime_height || x < start || x >= start + 3 * panel.runtime_width) return null;
      tier = Math.floor((x - start) / panel.runtime_width);
    }
    const spell = menu.spells.find(row => row.menu_slot === slot);
    const admitted = spell?.acquired && spell.holder_actor !== undefined && spell.menu_help?.definition_enabled && tier <= spell.unlocked_tier;
    return { hoveredSlot: slot, hoveredTier: tier, spellId: admitted ? spell.id : null, tier: admitted ? tier : null };
  }

  /** Returns a fresh top-down 640×480 indexed frame; never mutates scene history. */
  compose(scene: Uint8Array, paletteRGB8: Uint8Array, state: OriginalHudState, coverage?: Uint8Array): Uint8Array {
    if (scene.length !== 64000 || paletteRGB8.length !== 768) throw new Error('Invalid Original HUD frame');
    if (coverage && coverage.length !== 640 * 480) throw new Error('Invalid Original HUD coverage');
    coverage?.fill(0);
    const result = scene.slice();
    const sceneCoverage = coverage ? new Uint8Array(64000) : null;
    const scenePixel = (pos: number, color: number) => { result[pos] = color; if (sceneCoverage) sceneCoverage[pos] = 255; };
    const fullMap = state.world_map;
    const map = fullMap ?? state.minimap;
    const hud = state.hud;
    if (map && hud) {
      const minimap = fullMap ? null : state.minimap;
      if (minimap && (minimap.terrain.length !== 4096 || minimap.coverage.length !== 4096)) throw new Error('Invalid Original minimap dimensions');
      const mapWidth = fullMap ? 191 : 64, mapHeight = fullMap ? 200 : 64, scale = fullMap ? 1 : 2;
      const sourceAperture = (sourceX: number, sourceY: number) => {
        const phase = sourceY < 64 ? 63 - sourceY : sourceY - 64;
        const half = (64 * this.aperture[phase * 4]!) >> 8;
        return sourceX >= 64 - half && sourceX < 64 + half;
      };
      const aperture = (x: number, y: number) => sourceAperture(x * 2, y * 2);
      if (minimap) for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) {
        const i = y * 64 + x, pos = y * 320 + x, terrain = minimap.terrain[i]!;
        if (!Number.isInteger(terrain) || terrain < 0 || terrain > 255 || (minimap.coverage[i] !== 0 && minimap.coverage[i] !== 1)) throw new Error('Invalid Original minimap pixels');
        if (aperture(x, y)) {
          if (state.panels_opaque) scenePixel(pos, terrain);
          else if (minimap.coverage[i]) scenePixel(pos, this.tables.normal[(result[pos]! << 8) | terrain]!);
        }
      }
      const bound = (row: { generation: number | string; tick: number | string }) => {
        if (String(row.generation) !== String(hud.generation) || String(row.tick) !== String(hud.tick)) throw new Error('Stale Original minimap row');
      };
      const labels = map.labels ?? [];
      let nextLabel = 0;
      for (const row of map.entities) {
        bound(row); if (!row.visible) continue;
        const label = labels[nextLabel];
        const drawsName = label !== undefined && label.source_order === row.source_order;
        if (drawsName) {
          nextLabel++;
          if (label.subject_entity_index !== row.subject_entity_index || label.x !== row.x || label.y !== row.y ||
              label.palette_index !== row.palette_index || row.kind !== 1) throw new Error('Beyond Sight name differs from its player dot');
          if (row.x < 0 || row.x >= mapWidth * scale || row.y < 0 || row.y >= mapHeight * scale ||
              (!fullMap && !sourceAperture(row.x, row.y))) continue;
          scenePixel(Math.trunc(row.y / scale) * 320 + Math.trunc(row.x / scale), row.palette_index);
          // DrawText follows each admitted rival's dot. Its glyphs clip to the screen.
          let cursor = Math.trunc((row.x + 4) / scale);
          const top = Math.trunc(row.y / scale);
          for (const character of label.text) {
            const glyph = this.mapFont.get(character.charCodeAt(0) + 1);
            if (!glyph) throw new Error('Beyond Sight name has no Original FONT1 glyph');
            for (let y = 0; y < glyph.height && top + y < 200; y++)
              for (let x = 0; x < glyph.width && cursor + x < 320; x++)
                if (this.blob[glyph.mask.offset + y * glyph.width + x])
                  scenePixel((top + y) * 320 + cursor + x, row.palette_index);
            cursor += glyph.width;
          }
          continue;
        }
        if (row.kind !== 1 && row.kind !== 2 && row.kind !== 3) throw new Error('Invalid Original minimap entity');
        const symbol = row.kind === 2 ? this.symbols.get(row.selector) : undefined;
        if (row.kind === 2 && !symbol) continue;
        let x0 = Math.trunc(row.x / scale), y0 = Math.trunc(row.y / scale);
        const w = symbol?.width ?? (row.kind === 3 ? 1 : Math.ceil(Math.max(1, row.size) / scale));
        const h = symbol?.height ?? w;
        if (symbol && row.anchor === 3) x0 -= Math.trunc(w / 2);
        if (symbol && (row.anchor === 2 || row.anchor === 3)) y0 -= h;
        for (let dy = 0; dy < h; dy++) for (let dx = 0; dx < w; dx++) {
          const x = x0 + dx, y = y0 + dy, si = dy * w + dx;
          if (x < 0 || x >= mapWidth || y < 0 || y >= mapHeight || (!fullMap && !aperture(x, y)) || (symbol && !this.blob[symbol.mask.offset + si])) continue;
          const pos = y * 320 + x, color = symbol ? this.blob[symbol.indices.offset + si]! : row.palette_index;
          scenePixel(pos, row.kind === 3 ? this.tables.normal[(result[pos]! << 8) | color]! : color);
        }
      }
      if (nextLabel !== labels.length) throw new Error('Beyond Sight name has no matching player dot');
      if (fullMap) {
        const cross = (x: number, y: number) => { const pos = y * 320 + x; scenePixel(pos, this.tables.normal[(this.mapCrossColor << 8) | result[pos]!]!); };
        cross(94, 100);
        for (let arm = 1; arm <= 15; arm++) { cross(94 - arm, 100); cross(94 + arm, 100); cross(94, 100 - arm); cross(94, 100 + arm); }
      }
      for (let ordinal = 0; ordinal < map.marks.length; ordinal++) {
        const mark = map.marks[ordinal]!;
        if (mark.kind === 3) {
          const vertices = map.marks.slice(ordinal, ordinal + 7);
          if (vertices.length !== 7) throw new Error('Incomplete Original minimap arrow');
          for (let i = 0; i < 7; i++) {
            const row = vertices[i]!; bound(row);
            if (row.kind !== 3 || row.source_order !== mark.source_order + i || row.visible !== mark.visible || row.palette_index !== mark.palette_index) throw new Error('Invalid Original minimap arrow');
          }
          if (mark.visible) for (let i = 0; i < 7; i++) {
            const from = vertices[i]!, to = vertices[(i + 1) % 7]!;
            let x = Math.trunc(from.x / scale), y = Math.trunc(from.y / scale);
            const x1 = Math.trunc(to.x / scale), y1 = Math.trunc(to.y / scale);
            const dx = Math.abs(x1 - x), sx = x < x1 ? 1 : -1, dy = -Math.abs(y1 - y), sy = y < y1 ? 1 : -1;
            let err = dx + dy;
            for (;;) {
              if (x >= 0 && x < mapWidth && y >= 0 && y < mapHeight) scenePixel(y * 320 + x, mark.palette_index);
              if (x === x1 && y === y1) break;
              const twice = 2 * err;
              if (twice >= dy) { err += dy; x += sx; }
              if (twice <= dx) { err += dx; y += sy; }
            }
          }
          ordinal += 6; continue;
        }
        bound(mark); if (!mark.visible || mark.kind !== 2) continue;
        const x = Math.trunc(mark.x / scale), y = Math.trunc(mark.y / scale);
        if (x < 3 || x >= mapWidth - 3 || y < 3 || y >= mapHeight - 3) continue;
        const sourceY = y * 2, phase = sourceY < 64 ? 63 - sourceY : sourceY - 64;
        const half = fullMap ? 0 : (32 * this.aperture[phase * 4]!) >> 8;
        if (!fullMap && (x < 32 - half + 3 || x >= 32 + half - 3)) continue;
        for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) if (dx || dy) scenePixel((y + dy) * 320 + x + dx, mark.palette_index);
      }
    }
    // Unity's source layout is 640×480, scaled against a 320×200 source world.
    const screen = new Uint8Array(640 * 480);
    for (let y = 0; y < 480; y++) for (let x = 0; x < 640; x++) screen[y * 640 + x] = result[Math.floor(y * 200 / 480) * 320 + Math.floor(x / 2)]!;
    if (coverage && sceneCoverage) for (let y = 0; y < 480; y++) for (let x = 0; x < 640; x++) coverage[y * 640 + x] = sceneCoverage[Math.floor(y * 200 / 480) * 320 + Math.floor(x / 2)]!;
    const blend = this.tables.normal;
    const pixel = (x: number, y: number, index: number, transparent = false) => {
      if (x < 0 || x >= 640 || y < 0 || y >= 480) return;
      const pos = y * 640 + x;
      screen[pos] = transparent ? blend[(screen[pos]! << 8) | index]! : index;
      if (coverage) coverage[pos] = 255;
    };
    const sprite = (selector: number, x: number, y: number, transparent = false, tint?: number) => {
      const row = this.sprites.get(selector);
      if (!row) throw new Error(`Missing Original HUD selector ${selector}`);
      for (let dy = 0; dy < row.runtime_height; dy++) for (let dx = 0; dx < row.runtime_width; dx++) {
        const i = Math.floor(dy * row.height / row.runtime_height) * row.width + Math.floor(dx * row.width / row.runtime_width);
        if (this.blob[row.mask.offset + i] !== 0) pixel(x + dx, y + dy, tint ?? this.blob[row.indices.offset + i]!, transparent);
      }
    };
    const fill = (x: number, y: number, width: number, height: number, color: number) => {
      for (let dy = 0; dy < height; dy++) for (let dx = 0; dx < width; dx++) pixel(x + dx, y + dy, color);
    };
    const width = (value: number, denominator: number) => denominator <= 0 ? 0 : Math.max(0, Math.min(64, Math.trunc(value * 64 / denominator)));
    if (hud && !fullMap && state.top_bar_visible !== false) {
      const hand = (slot: OriginalHudSlot, x: number) => {
        if (!slot.holder_entity_index || !slot.draw_admitted) return;
        sprite(slot.frame_selector, x, 2, !state.panels_opaque);
        sprite(slot.icon_selector, x, 2);
        const row = this.sprites.get(slot.frame_selector)!;
        const tier = this.tiers.get(slot.level_text_selector);
        if (tier) {
          const tierX = x + row.runtime_width - tier.width - 2;
          for (let dy = 0; dy < tier.height; dy++) for (let dx = 0; dx < tier.width; dx++) {
            if (this.blob[tier.rgba.offset + (dy * tier.width + dx) * 4 + 3]) pixel(tierX + dx, 2 + dy, tier.color);
          }
        }
        if (slot.max_mana > 0) {
          fill(x + 4, 38, Math.trunc(56 * (slot.caster_mana % slot.max_mana) / slot.max_mana), 4, hud.mana_max_palette_index);
          let charges = Math.trunc(slot.caster_mana / slot.max_mana);
          for (let col = 0; col < 27 && charges > 0; col++) for (let row = 0; row < 2 && charges > 0; row++, charges--) fill(x + 2 * (col + 2), 38 + 2 * row, 2, 2, hud.mana_palette_index);
        }
      };
      hand(hud.left, 510); hand(hud.right, 574);
      const minimapWidth = this.sprites.get(hud.minimap_panel_selector)?.runtime_width;
      const panelWidth = this.sprites.get(hud.castle_panel_selector)?.runtime_width;
      if (minimapWidth === undefined || panelWidth === undefined) throw new Error('Missing Original top status panel');
      const castleX = 2 + minimapWidth, balloonX = castleX + panelWidth, playerX = balloonX + panelWidth;
      sprite(hud.minimap_panel_selector, 2, 2, !state.panels_opaque);
      const status = state.source_top_status;
      if (status && (status.castle_rows.length > 1 || status.balloons.length !== 3)) throw new Error('Invalid Original Castle status');
      const castle = status?.castle_rows[0];
      sprite(castle?.panel_selector ?? hud.castle_panel_selector, castleX, 2, !state.panels_opaque);
      if (castle && castle.stage !== 0) {
        sprite(castle.icon_selector, castleX + 2, 2);
        sprite(hud.life_mana_icon_selector, castleX + 38, 2);
        const barX = castleX + 58, total = castle.mana + castle.owner_mana_aggregate;
        fill(barX, 10, width(castle.life, castle.max_life), 10, hud.life_palette_index);
        if (total !== castle.max_mana) fill(barX, 28, width(castle.max_mana, hud.global_mana_denominator), 10, hud.mana_max_palette_index);
        const full = total === castle.max_mana && hud.color_phase_2 === 1;
        fill(barX, 28, width(full ? castle.max_mana : total, hud.global_mana_denominator), 10, full ? hud.mana_max_palette_index : hud.mana_palette_index);
        if (status && status.mana_goal_percent !== 0) {
          const goalX = barX + Math.trunc((status.mana_goal_percent * 64) / 100);
          fill(goalX, 26, 2, 2, status.goal_palette_index); fill(goalX, 38, 2, 2, status.goal_palette_index);
        }
      }
      sprite(status?.balloon_panel_selector ?? hud.balloon_panel_selector, balloonX, 2, !state.panels_opaque);
      if (castle && status) {
        sprite(status.balloon_icon_selector, balloonX + 2, 2);
        sprite(hud.life_mana_icon_selector, balloonX + 38, 2);
        const count = castle.stage <= 3 ? (castle.stage === 0 ? 0 : 1) : castle.stage <= 5 ? 2 : 3;
        for (let i = 0; i < count; i++) {
          const balloon = status.balloons[i]!; if (!balloon.present) continue;
          if (balloon.life >= 0) fill(balloonX + 58, 12 + 2 * i, width(balloon.life, balloon.max_life), 2, hud.life_palette_index);
          fill(balloonX + 58, 30 + 2 * i, width(balloon.mana, balloon.max_mana), 2, hud.mana_palette_index);
        }
      }
      sprite(hud.player_panel_selector, playerX, 2, !state.panels_opaque);
      sprite(hud.player_icon_selector, playerX + 2, 2);
      sprite(hud.life_mana_icon_selector, playerX + 38, 2);
      fill(playerX + 58, 10, width(hud.local_player_life, hud.local_player_max_life), 10, hud.life_palette_index);
      fill(playerX + 58, 28, width(hud.local_player_max_mana, hud.global_mana_denominator), 10, hud.mana_max_palette_index);
      fill(playerX + 58, 28, width(hud.local_player_mana, hud.global_mana_denominator), 10, hud.mana_palette_index);
    }
    const menu = state.spell_menu;
    if (menu?.open) {
      if (menu.hovered_slot < 0 || menu.hovered_slot >= 26 || menu.hovered_tier < 0 || menu.hovered_tier > 2 || menu.wizard_colors.length !== 2) throw new Error('Invalid Original spell menu');
      const transparent = !menu.panels_opaque;
      sprite(88, 0, 408, transparent); sprite(88, 632, 408, transparent);
      for (let index = 0; index < 26; index++) {
        const x = 8 + (index % 13) * 48, y = 408 + Math.floor(index / 13) * 36;
        const spell = menu.spells.find(row => row.menu_slot === index), help = spell?.menu_help;
        if (!spell || !help) throw new Error('Missing Original spell menu observation');
        const tier = index === menu.hovered_slot ? menu.hovered_tier : help.selected_tier;
        if (!help.definition_enabled) sprite(89, x, y, transparent);
        else if (help.is_standard) {
          if (!help.blink_hidden) {
            const allowed = !help.castle_blocked[tier];
            if (allowed && index === menu.hovered_slot) {
              const cost = help.tier_mana_costs[tier]!;
              if (cost > 0) {
                sprite(87, x, y);
                fill(x + 6, y + 28, Math.trunc(36 * (menu.mana % cost) / cost), 4, menu.wizard_colors[1]!);
                let shots = Math.trunc(menu.mana / cost);
                for (let sx = 0; sx < 36 && shots > 0; sx += 2) for (let sy = 0; sy < 4 && shots > 0; sy += 2, shots--) fill(x + 6 + sx, y + 28 + sy, 2, 2, menu.wizard_colors[0]!);
              }
            } else sprite(allowed ? 89 : 91, x, y, transparent);
            if (!spell.menu_artwork) throw new Error('Missing Original spell artwork');
            sprite(spell.menu_artwork.grid_icon, x, y, !allowed);
          }
          if (help.blink_hidden || index === menu.hovered_slot) {
            if (spell.marked_hand === 'left') sprite(149, x, y, true);
            if (spell.marked_hand === 'right') {
              const marker = this.sprites.get(149); if (!marker) throw new Error('Missing Original hand marker');
              sprite(150, x + 48 - marker.runtime_width, y, true);
            }
          }
        } else {
          sprite(89, x, y, transparent);
          if (help.discovered && spell.menu_artwork) sprite(spell.menu_artwork.grid_icon, x, y, false, 0xa6);
        }
      }
      sprite(90, 8 + menu.hovered_slot % 13 * 48, 408 + Math.floor(menu.hovered_slot / 13) * 36);
      const hovered = menu.spells.find(row => row.menu_slot === menu.hovered_slot);
      if (hovered?.menu_help?.is_standard && hovered.menu_artwork) {
        const row = this.sprites.get(163); if (!row) throw new Error('Missing Original spell tier panel');
        const start = Math.max(0, Math.min(640 - 3 * row.runtime_width, 8 + menu.hovered_slot % 13 * 48 + 24 - Math.trunc(3 * row.runtime_width / 2)));
        for (let tier = 0; tier < 3; tier++) {
          const x = start + tier * row.runtime_width, y = 408 - row.runtime_height;
          const unlocked = tier <= hovered.unlocked_tier, allowed = !hovered.menu_help.castle_blocked[tier];
          const cost = hovered.menu_help.tier_mana_costs[tier]!;
          sprite(!unlocked ? 163 : allowed && cost > 0 && menu.mana >= cost ? 161 : 162, x, y);
          if (unlocked) {
            sprite(165 + tier, x + 6, y + 10);
            sprite(hovered.menu_artwork.tier_icons[tier]!, x + 18, y + 6, !allowed);
            if (tier < 2) {
              let progress = 54;
              if (tier === hovered.unlocked_tier) {
                const delta = hovered.menu_help.experience_thresholds[tier + 1]! - hovered.menu_help.experience_thresholds[tier]!;
                progress = delta > 0 ? Math.max(0, Math.min(54, Math.trunc(54 * (hovered.menu_help.experience - hovered.menu_help.experience_thresholds[tier]!) / delta))) : 0;
              }
              fill(x + 6, y + 28, 54, 2, this.objectiveBackground); fill(x + 6, y + 28, progress, 2, 119);
            }
          }
          if (tier === menu.hovered_tier) sprite(164, x, y);
        }
      }
    }
    const drawPlayerInformation = () => {
      if (!state.player_information) return;
      const board = state.player_information, large = this.symbols.get(85), mini = this.symbols.get(86);
      if (board.projection_error || !large || !mini || !this.gameplayFont.size || board.players.length > 8 ||
          board.players.some((p, i) => p.source_slot < 0 || p.source_slot >= 8 || (i > 0 && p.source_slot <= board.players[i - 1]!.source_slot) ||
            !p.display_name || [...p.display_name].some(c => c.charCodeAt(0) === 0 || c.charCodeAt(0) > 255) || p.kills8.length !== 8))
        throw new Error('Original player information unavailable');
      const tile = (row: SymbolRow, x: number, y: number) => {
        for (let dy = 0; dy < row.height * 2; dy++) for (let dx = 0; dx < row.width * 2; dx++) {
          const i = Math.floor(dy / 2) * row.width + Math.floor(dx / 2);
          if (this.blob[row.mask.offset + i]) pixel(x + dx, y + dy, this.blob[row.indices.offset + i]!);
        }
      };
      const outline = (x: number, y: number, w: number, h: number, color: number) => {
        fill(x, y, w, 1, color); fill(x, y + h - 1, w, 1, color);
        fill(x, y + 1, 1, h - 2, color); fill(x + w - 1, y + 1, 1, h - 2, color);
      };
      const text = (value: string, x: number, y: number, color: number) => {
        for (const ch of value) {
          const code = ch.charCodeAt(0), glyph = this.gameplayFont.get(code + 1);
          if (!glyph) throw new Error('Missing Original gameplay glyph');
          if (code !== 32 && code !== 9) for (let gy = 0; gy < glyph.height; gy++) for (let gx = 0; gx < glyph.width; gx++)
            if (this.blob[glyph.mask.offset + gy * glyph.width + gx]) pixel(x + gx, y + gy, color);
          x += code === 32 || code === 9 ? this.gameplayFont.get(33)!.width : glyph.width;
        }
      };
      const lw = large.width * 2, lh = large.height * 2, mw = mini.width * 2, mh = mini.height * 2;
      const x = Math.trunc((640 - (board.players.length * mw + mw)) / 2);
      let y = Math.trunc((480 - board.players.length * lh) / 2);
      for (const player of board.players) {
        tile(large, x, y); outline(x + 4, y + 4, lw - 8, lh - 8, player.outline_palette_index);
        text(player.display_name, x + 8, y + 6, player.text_palette_index);
        text(String(player.max_mana), x + 8, y + 20, player.text_palette_index);
        let mx = x + lw;
        for (const opponent of board.players) {
          tile(mini, mx, y);
          const own = opponent.source_slot === player.source_slot;
          outline(mx + 4, y + 4, mw - 8, mh - 8, own ? this.objectiveBackground : opponent.outline_palette_index);
          if (!own) {
            const kills = player.kills8[opponent.source_slot]!;
            const formatted = kills < 0 ? '-' + String(-kills).padStart(2, '0') : String(kills).padStart(3, '0');
            text(formatted, mx + 8, y + 10, opponent.text_palette_index);
          }
          mx += mw;
        }
        y += mh;
      }
    };
    if (state.pause_menu?.open) {
      // DrawPauseMenu_2FD90; selectors 175–177 use source twice-shading when disabled.
      const baseX = 268, baseY = 76;
      const entries = [
        { selector: 174, x: baseX, y: baseY, enabled: true },
        { selector: 175, x: baseX, y: baseY + 32, enabled: state.pause_menu.can_load },
        { selector: 176, x: baseX, y: baseY + 64, enabled: state.pause_menu.sound_enabled },
        { selector: 177, x: baseX + 51, y: baseY + 64, enabled: state.pause_menu.music_enabled },
        { selector: 178, x: baseX, y: baseY + 96, enabled: true },
      ];
      for (const entry of entries) {
        sprite(entry.selector, entry.x, entry.y);
        if (!entry.enabled) {
          const row = this.sprites.get(entry.selector)!;
          // Load shades the 102×32 area; volume buttons shade their source extent.
          for (let y = entry.y; y < entry.y + row.runtime_height; y++) for (let x = entry.x; x < entry.x + row.runtime_width; x++) {
            const pos = y * 640 + x;
            screen[pos] = blend[(this.objectiveBackground << 8) | screen[pos]!]!;
            screen[pos] = blend[(this.objectiveBackground << 8) | screen[pos]!]!;
            if (coverage) coverage[pos] = 255;
          }
        }
      }
    }
    if (state.objective_visible && state.objective_selector != null) {
      const row = this.objectives.get(state.objective_selector);
      if (!row) throw new Error(`Missing Original objective ${state.objective_selector}`);
      // Source background uses TABLESN[CLRN[0] << 8 | destination].
      for (let y = row.blend_y; y < row.blend_y + row.blend_height; y++) for (let x = row.blend_x; x < row.blend_x + row.blend_width; x++) {
        const at = y * 640 + x; screen[at] = blend[(this.objectiveBackground << 8) | screen[at]!]!;
        if (coverage) coverage[at] = 255;
      }
      for (let y = 0; y < row.render_height; y++) for (let x = 0; x < row.render_width; x++) {
        const i = y * row.render_width + x;
        if (this.blob[row.mask.offset + i]) pixel(row.reference_x + x, row.reference_y + y, this.blob[row.indices.offset + i]!);
      }
    }
    drawPlayerInformation();
    const inspected = state.exit_inspection_subject ? this.exitMarker(state, state.exit_inspection_subject) : undefined;
    const automatic = this.hintDraw;
    const help = inspected ? this.exitHelp : automatic ? this.mapHelp.get(automatic.row) : undefined;
    if (help && (inspected || automatic)) {
      const [corner, side, horizontal] = help.frame;
      if (!corner || !side || !horizontal) throw new Error('Missing Original exit help frame');
      // Explicit map inspection uses the source Type-8 leader and owner font;
      // the live WebGL scene receives an opaque source-colour box.
      const anchorX = inspected ? inspected.x * 2 : automatic!.x;
      const anchorY = inspected ? Math.ceil(inspected.y * 480 / 200) : automatic!.y;
      const boxWidth = help.box_width ?? help.text.length * 8, boxHeight = help.height;
      if (boxWidth <= 0 || boxWidth > 220 || help.width > boxWidth || help.height > boxHeight)
        throw new Error('Invalid Original exit help text dimensions');
      const centerX = 600 < anchorX + 135 ? anchorX - 135 : anchorX + 25;
      const centerY = anchorY - 94 + 28 < 80 ? anchorY + 66 : anchorY - 94;
      const textX = Math.min(600 - boxWidth, Math.max(40 + corner.width, centerX - Math.floor(boxWidth / 2)));
      const textY = Math.min(260 - boxHeight, Math.max(80 + corner.height, centerY - Math.floor(boxHeight / 2)));
      const frameWidth = horizontal.width * Math.ceil((boxWidth + 8) / horizontal.width);
      const frameHeight = Math.ceil(boxHeight / side.height) * side.height + 2 * (corner.height - horizontal.height);
      const frameX = textX - Math.floor((frameWidth - boxWidth) / 2);
      const frameY = textY - Math.floor((frameHeight - boxHeight) / 2);
      const lineX = anchorX < frameX - 30 ? frameX - 2 : anchorX > frameX + frameWidth + 30 ? frameX + frameWidth : textX + Math.floor(boxWidth / 2);
      const lineY = anchorY < frameY - 30 ? frameY - 2 : anchorY > frameY + frameHeight + 30 ? frameY + frameHeight : textY + Math.floor(boxHeight / 2);
      const line = (x0: number, y0: number, x1: number, y1: number, color: number) => {
        if (y0 > y1) { [x0, x1] = [x1, x0]; [y0, y1] = [y1, y0]; }
        const dx = Math.abs(x1 - x0), dy = y1 - y0, direction = x1 < x0 ? -1 : 1;
        const major = Math.max(dx, dy), minor = Math.min(dx, dy); let error = 2 * minor - major;
        pixel(x0, y0, color);
        for (let i = 0; i < major; i++) {
          if (dx >= dy) x0 += direction; else y0++;
          if (error >= 0) { if (dx >= dy) y0++; else x0 += direction; error += 2 * (minor - major); }
          else error += 2 * minor;
          pixel(x0, y0, color);
        }
      };
      if (lineX !== textX + Math.floor(boxWidth / 2) || lineY !== textY + Math.floor(boxHeight / 2)) {
        line(anchorX, anchorY, lineX, lineY, help.line_color);
        line(anchorX + 1, anchorY, lineX + 1, lineY, help.color);
      }
      fill(frameX, frameY, frameWidth, frameHeight, help.background);
      const frameSprite = (row: SymbolRow, x: number, y: number) => {
        for (let dy = 0; dy < row.height; dy++) for (let dx = 0; dx < row.width; dx++) {
          const i = dy * row.width + dx;
          if (this.blob[row.mask.offset + i]) pixel(x + dx, y + dy, this.blob[row.indices.offset + i]!);
        }
      };
      const top = frameY - horizontal.height, bottom = frameY + frameHeight;
      for (const x of [frameX - side.width, frameX + frameWidth]) {
        frameSprite(corner, x, top);
        for (let y = 0; y < boxHeight / 14; y++) frameSprite(side, x, top + corner.height + y * side.height);
        frameSprite(corner, x, top + corner.height + boxHeight / 14 * side.height);
      }
      for (let x = frameX; x < frameX + frameWidth; x += horizontal.width) {
        frameSprite(horizontal, x, top); frameSprite(horizontal, x, bottom);
      }
      for (let y = 0; y < help.height; y++) for (let x = 0; x < help.width; x++)
        if (this.blob[help.mask.offset + y * help.width + x]) pixel(textX + x, textY + y, help.color);
    }
    return screen;
  }
}
