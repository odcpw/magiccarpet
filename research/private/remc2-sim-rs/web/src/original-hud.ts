/** Indexed Original HUD. All artwork and dynamic selectors come from the local owner cache / Rust. */
import type { IndexedBlendTables } from './original-adapter';

type Span = { offset: number; length: number };
type SpriteRow = { selector: number; width: number; height: number; runtime_width: number; runtime_height: number; indices: Span; mask: Span };
type ObjectiveRow = { selector: number; reference_x: number; reference_y: number; render_width: number; render_height: number; blend_x: number; blend_y: number; blend_width: number; blend_height: number; indices: Span; mask: Span };
type TierRow = { tier: number; width: number; height: number; color: number; rgba: Span };
type SymbolRow = { selector: number; width: number; height: number; indices: Span; mask: Span };
type Manifest = { schema: string; bytes: number; normal: Span; exit: Span; aperture: Span; objective_background: number; map_cross_color: number; sprites: SpriteRow[]; symbols: SymbolRow[]; objectives: ObjectiveRow[]; tiers: TierRow[] };

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
  minimap?: { terrain: number[]; coverage: number[]; entities: MinimapEntity[]; marks: MinimapMark[] } | null;
  world_map?: { entities: MinimapEntity[]; marks: MinimapMark[] } | null;
  source_top_status?: SourceTopStatus | null;
  spell_menu?: SpellMenu | null;
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
type MinimapEntity = { generation: number | string; tick: number | string; x: number; y: number; selector: number; palette_index: number; kind: number; size: number; anchor: number; visible: number };
type MinimapMark = { generation: number | string; tick: number | string; source_order: number; x: number; y: number; palette_index: number; kind: number; visible: number };

export class OriginalHud {
  readonly width = 640;
  readonly height = 480;
  readonly tables: IndexedBlendTables;
  private readonly sprites = new Map<number, SpriteRow>();
  private readonly symbols = new Map<number, SymbolRow>();
  private readonly objectives = new Map<number, ObjectiveRow>();
  private readonly tiers = new Map<number, TierRow>();
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
    for (const row of manifest.objectives) {
      if (span(row.indices).length !== row.render_width * row.render_height || span(row.mask).length !== row.render_width * row.render_height) throw new Error('Invalid Original objective');
      this.objectives.set(row.selector, row);
    }
    for (const row of manifest.tiers) {
      if (span(row.rgba).length !== row.width * row.height * 4) throw new Error('Invalid Original tier');
      this.tiers.set(row.tier, row);
    }
  }
  static async load(base = '/original-api'): Promise<OriginalHud> {
    const [manifestResponse, blobResponse] = await Promise.all([
      fetch(`${base}/assets/manifest`, { signal: AbortSignal.timeout(15000) }),
      fetch(`${base}/assets/blob`, { signal: AbortSignal.timeout(15000) }),
    ]);
    if (!manifestResponse.ok || !blobResponse.ok) throw new Error('Original owner HUD assets unavailable');
    return new OriginalHud(new Uint8Array(await blobResponse.arrayBuffer()), await manifestResponse.json() as Manifest);
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
  compose(scene: Uint8Array, paletteRGB8: Uint8Array, state: OriginalHudState): Uint8Array {
    if (scene.length !== 64000 || paletteRGB8.length !== 768) throw new Error('Invalid Original HUD frame');
    const result = scene.slice();
    const fullMap = state.world_map;
    const map = fullMap ?? state.minimap;
    const hud = state.hud;
    if (map && hud) {
      const minimap = fullMap ? null : state.minimap;
      if (minimap && (minimap.terrain.length !== 4096 || minimap.coverage.length !== 4096)) throw new Error('Invalid Original minimap dimensions');
      const mapWidth = fullMap ? 191 : 64, mapHeight = fullMap ? 200 : 64, scale = fullMap ? 1 : 2;
      const aperture = (x: number, y: number) => {
        const sourceY = y * 2, phase = sourceY < 64 ? 63 - sourceY : sourceY - 64;
        const half = (64 * this.aperture[phase * 4]!) >> 8;
        return x * 2 >= 64 - half && x * 2 < 64 + half;
      };
      if (minimap) for (let y = 0; y < 64; y++) for (let x = 0; x < 64; x++) {
        const i = y * 64 + x, pos = y * 320 + x, terrain = minimap.terrain[i]!;
        if (!Number.isInteger(terrain) || terrain < 0 || terrain > 255 || (minimap.coverage[i] !== 0 && minimap.coverage[i] !== 1)) throw new Error('Invalid Original minimap pixels');
        if (aperture(x, y)) {
          if (state.panels_opaque) result[pos] = terrain;
          else if (minimap.coverage[i]) result[pos] = this.tables.normal[(result[pos]! << 8) | terrain]!;
        }
      }
      const bound = (row: { generation: number | string; tick: number | string }) => {
        if (String(row.generation) !== String(hud.generation) || String(row.tick) !== String(hud.tick)) throw new Error('Stale Original minimap row');
      };
      for (const row of map.entities) {
        bound(row); if (!row.visible) continue;
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
          result[pos] = row.kind === 3 ? this.tables.normal[(result[pos]! << 8) | color]! : color;
        }
      }
      if (fullMap) {
        const cross = (x: number, y: number) => { const pos = y * 320 + x; result[pos] = this.tables.normal[(this.mapCrossColor << 8) | result[pos]!]!; };
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
              if (x >= 0 && x < mapWidth && y >= 0 && y < mapHeight) result[y * 320 + x] = mark.palette_index;
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
        for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) if (dx || dy) result[(y + dy) * 320 + x + dx] = mark.palette_index;
      }
    }
    // Unity's source layout is 640×480, scaled against a 320×200 source world.
    const screen = new Uint8Array(640 * 480);
    for (let y = 0; y < 480; y++) for (let x = 0; x < 640; x++) screen[y * 640 + x] = result[Math.floor(y * 200 / 480) * 320 + Math.floor(x / 2)]!;
    const blend = this.tables.normal;
    const pixel = (x: number, y: number, index: number, transparent = false) => {
      if (x < 0 || x >= 640 || y < 0 || y >= 480) return;
      const pos = y * 640 + x;
      screen[pos] = transparent ? blend[(screen[pos]! << 8) | index]! : index;
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
      }
      for (let y = 0; y < row.render_height; y++) for (let x = 0; x < row.render_width; x++) {
        const i = y * row.render_width + x;
        if (this.blob[row.mask.offset + i]) pixel(row.reference_x + x, row.reference_y + y, this.blob[row.indices.offset + i]!);
      }
    }
    return screen;
  }
}
