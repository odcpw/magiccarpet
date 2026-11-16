import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { OriginalHud, type OriginalHudState } from '../src/original-hud.ts';

const run = spawnSync('python3', ['-c', `import base64,json,sys
sys.path.insert(0,'${new URL('..', import.meta.url).pathname}')
from original_assets import OriginalAssets
x=OriginalAssets()
print(json.dumps({'manifest': x.manifest, 'blob': base64.b64encode(x.blob).decode()}))`], { encoding: 'utf8', maxBuffer: 4_000_000 });
if (run.status !== 0) throw new Error(run.stderr);
const assets = JSON.parse(run.stdout);
const blob = Uint8Array.from(Buffer.from(assets.blob, 'base64'));
const savedFetch = globalThis.fetch;
globalThis.fetch = async (input) => String(input).endsWith('/manifest')
  ? new Response(JSON.stringify(assets.manifest), { headers: { 'Content-Type': 'application/json' } })
  : new Response(blob);
const hud = await OriginalHud.load('/original-api');
globalThis.fetch = savedFetch;

const slot = {holder_entity_index: 1, draw_admitted: 1, frame_selector: 1, icon_selector: 123,
  level_text_selector: 0, max_mana: 100, caster_mana: 30};
const state: OriginalHudState = { panels_opaque: true, hud: { generation: 5, tick: 3,
  minimap_panel_selector: 40, castle_panel_selector: 54, balloon_panel_selector: 54,
  player_panel_selector: 41, player_icon_selector: 43, life_mana_icon_selector: 42,
  local_player_life: 50, local_player_max_life: 100, local_player_mana: 10,
  local_player_max_mana: 100, global_mana_denominator: 100, color_phase_2: 0,
  life_palette_index: 117, mana_palette_index: 118, mana_max_palette_index: 119,
  left: slot, right: slot } };

test('owner indexed HUD preserves scene and reflects native life and objective admission', () => {
  const scene = new Uint8Array(64000).fill(7), palette = new Uint8Array(768);
  const image = hud.compose(scene, palette, state);
  assert.equal(image.length, 640 * 480); assert.equal(scene[0], 7);
  assert.equal(image[10 * 640 + 440], 117);
  assert.notEqual(image[10 * 640 + 480], 117);
  const reduced = hud.compose(scene, palette, { ...state, hud: { ...state.hud!, local_player_life: 0 } });
  assert.notEqual(reduced[10 * 640 + 440], 117);
  const objective = hud.compose(scene, palette, { ...state, objective_visible: true, objective_selector: 48 });
  assert.notDeepEqual(objective, image);
  assert.throws(() => hud.compose(scene, palette, { ...state, objective_visible: true, objective_selector: 999 }), /objective/);
});

test('minimap commands must match source frame; terrain admission is bounded', () => {
  const scene = new Uint8Array(64000).fill(7), palette = new Uint8Array(768);
  const minimap = { terrain: Array(4096).fill(39), coverage: Array(4096).fill(1), entities: [], marks: [] };
  const image = hud.compose(scene, palette, {...state, minimap});
  assert.equal(image[40 * 640 + 64], 39);
  assert.throws(() => hud.compose(scene, palette, {...state, minimap: {...minimap, terrain: [1]}}), /dimensions/);
  const bad = {...minimap, entities: [{generation: 6, tick: 3, x: 64, y: 64, selector: 0, palette_index: 40, kind: 1, size: 2, anchor: 0, visible: 1}]};
  assert.throws(() => hud.compose(scene, palette, {...state, minimap: bad}), /Stale/);
});

test('source spell hit testing returns only unlocked native-backed intents', () => {
  const row = { id: 'fireball', acquired: true, holder_actor: 9n, menu_slot: 0, unlocked_tier: 1, marked_hand: 'none',
    menu_artwork: {grid_icon: 97, tier_icons: [179, 180, 181]}, menu_help: {is_standard: true, selected_tier: 0,
      castle_blocked: [false, false, false], definition_enabled: true, blink_hidden: false, discovered: true,
      tier_mana_costs: [10, 20, 30], experience: 0, experience_thresholds: [0, 100, 200]} };
  const menu = {open: true, hovered_slot: 0, hovered_tier: 0, mana: 20, wizard_colors: [10, 11], panels_opaque: true, spells: [row]};
  assert.deepEqual(hud.hitTest(9, 410, menu), {hoveredSlot: 0, hoveredTier: 0, spellId: 'fireball', tier: 0});
  assert.equal(hud.hitTest(-1, 410, menu), null);
  const tierPanel = assets.manifest.sprites.find((item: {selector: number}) => item.selector === 163);
  const panelStart = Math.max(0, Math.min(640 - 3 * tierPanel.runtime_width, 32 - Math.trunc(3 * tierPanel.runtime_width / 2)));
  const locked = hud.hitTest(panelStart + 2 * tierPanel.runtime_width + 1, 408 - tierPanel.runtime_height + 1, menu);
  assert.deepEqual(locked, {hoveredSlot: 0, hoveredTier: 2, spellId: null, tier: null});
  assert.equal(hud.hitTest(9, 410, {...menu, spells: [{...row, acquired: false}]})?.spellId, null);
});

test('split world map retains native map scale and rejects stale rows', () => {
  const scene = new Uint8Array(64000).fill(7), palette = new Uint8Array(768);
  const mark = {generation: 5, tick: 3, source_order: 1, x: 25, y: 30, palette_index: 44, kind: 2, visible: 1};
  const image = hud.compose(scene, palette, {...state, world_map: {entities: [], marks: [mark]}});
  assert.equal(image[70 * 640 + 24 * 2], 44);
  assert.throws(() => hud.compose(scene, palette, {...state, world_map: {entities: [], marks: [{...mark, tick: 4}]}}), /Stale/);
});

test('source Castle and balloon rows control status bars', () => {
  const scene = new Uint8Array(64000).fill(7), palette = new Uint8Array(768);
  const status = {castle_rows: [{stage: 1, panel_selector: 41, icon_selector: 43, life: 40, max_life: 100,
    mana: 5, max_mana: 50, owner_mana_aggregate: 0}], balloon_panel_selector: 41, balloon_icon_selector: 43,
    balloons: [{present: true, life: 20, max_life: 100, mana: 0, max_mana: 50},
      {present: false, life: 0, max_life: 1, mana: 0, max_mana: 1},
      {present: false, life: 0, max_life: 1, mana: 0, max_mana: 1}], mana_goal_percent: 0, goal_palette_index: 0};
  const image = hud.compose(scene, palette, {...state, source_top_status: status});
  assert.equal(image[10 * 640 + 184], 117);
  assert.equal(image[12 * 640 + 312], 117);
  const empty = hud.compose(scene, palette, {...state, source_top_status: {...status, castle_rows: []}});
  assert.notEqual(empty[10 * 640 + 184], 117);
});
