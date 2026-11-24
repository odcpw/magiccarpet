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

test('corner Beyond Sight names follow the source aperture and low-resolution FONT1 placement', () => {
  const dot = { generation: 5, tick: 3, source_order: 1, subject_entity_index: 344, x: 65, y: 63,
    selector: 0, palette_index: 119, kind: 1, size: 1, anchor: 0, visible: 1 };
  const scene = new Uint8Array(64000).fill(7), palette = new Uint8Array(768);
  const draw = (x: number, y: number) => {
    const player = { ...dot, x, y };
    return hud.compose(scene, palette, { ...state, top_bar_visible: false, minimap: {
      entities: [player], marks: [], labels: [{ ...player, text: 'N' }],
      terrain: Array(4096).fill(7), coverage: Array(4096).fill(0)
    } });
  };
  const frame = draw(dot.x, dot.y);
  const glyph = assets.manifest.font1.find((g: { selector: number }) => g.selector === 'N'.charCodeAt(0) + 1);
  const left = Math.floor((dot.x + 4) / 2), top = Math.floor(dot.y / 2);
  for (let y = 0; y < glyph.height; y++) for (let x = 0; x < glyph.width; x++) {
    const pixel = 640 * Math.ceil((top + y) * 480 / 200) + 2 * (left + x);
    assert.equal(frame[pixel], blob[glyph.mask.offset + y * glyph.width + x] ? dot.palette_index : 7);
  }
  assert.ok(!draw(0, 0).includes(dot.palette_index), 'a wizard outside the aperture has no visible name');
});

test('explicit exit-map inspection follows the admitted source marker and uses owner help pixels', async () => {
  const generated = new URL('../../../generated/', import.meta.url).pathname;
  const capture = spawnSync('python3', ['-c', `import base64,json,sys
from pathlib import Path
sys.path.insert(0,'${new URL('..', import.meta.url).pathname}')
from original_assets import OriginalAssets
g=Path('${generated}')
x=OriginalAssets.from_package({'package_dir':str(g/'dragon-bee-20261105-integration/level5-owner-current/level5'),
 'owner_cache_path':str(g/'secret-lifecycle-20261111-source4-environment')},g/'exit-help-row83-20261124-candidate')
print(json.dumps({'manifest':x.manifest,'blob':base64.b64encode(x.blob).decode()}))`],
    { encoding: 'utf8', maxBuffer: 4_000_000 });
  if (capture.status !== 0) throw new Error(capture.stderr);
  const qualified = JSON.parse(capture.stdout);
  const bytes = Uint8Array.from(Buffer.from(qualified.blob, 'base64'));
  globalThis.fetch = async input => String(input).endsWith('/manifest')
    ? new Response(JSON.stringify(qualified.manifest)) : new Response(bytes);
  let dayHud: OriginalHud;
  try { dayHud = await OriginalHud.load('/original-api'); } finally { globalThis.fetch = savedFetch; }
  assert.equal(qualified.manifest.map_profile, 'day');
  assert.equal(qualified.manifest.help['83'].text, 'Fly here to exit the level');
  const marker = {generation: 5, tick: 3, subject_entity_index: 298, help_selector: 83,
    x: 67, y: 4, selector: 84, palette_index: 0, kind: 2, size: 0, anchor: 3, visible: 1};
  const visible: OriginalHudState = {...state, world_map: {entities: [marker], marks: []}, exit_inspection_allowed: true};
  assert.equal(dayHud.exitMarkerAt(visible, 134, 10), 298);
  assert.equal(dayHud.exitMarkerAt(visible, 250, 100), null);
  assert.equal(dayHud.exitInspectionText(visible, 298), qualified.manifest.help['83'].text);
  const scene = new Uint8Array(64000).fill(7), palette = new Uint8Array(768);
  const baseline = dayHud.compose(scene, palette, visible);
  const coverage = new Uint8Array(640 * 480);
  const inspected = dayHud.compose(scene, palette, {...visible, exit_inspection_subject: 298}, coverage);
  assert.notDeepEqual(inspected, baseline, 'inspection draws owner text and its source-frame leader');
  assert.equal(inspected[10 * 640 + 134], qualified.manifest.help['83'].line_color, 'leader reaches the map marker');
  assert.equal(coverage[10 * 640 + 134], 255);
  const moved = {...visible, world_map: {entities: [{...marker, x: 70, y: 8}], marks: []}};
  assert.equal(dayHud.exitMarkerAt(moved, 140, 20), 298);
  assert.notDeepEqual(dayHud.compose(scene, palette, {...moved, exit_inspection_subject: 298}), inspected);
  const stale = {...visible, world_map: {entities: [{...marker, generation: 6}], marks: []}};
  assert.equal(dayHud.exitInspectionText(stale, 298), null);
  assert.equal(dayHud.exitInspectionText({...visible, exit_inspection_allowed: false}, 298), null);
  assert.equal(dayHud.exitInspectionText({...visible, world_map: {entities: [], marks: []}}, 298), null);
  assert.equal(dayHud.exitMarkerAt({...visible, world_map: {entities: [{...marker, y: -1}], marks: []}}, 134, 0), null);

  assert.equal(qualified.manifest.help['19'].text, 'Destroy this for its mana');
  assert.equal(qualified.manifest.help['20'].text, 'Collect this spell by flying into it');
  const creature = {...marker, subject_entity_index: 10, help_selector: 19, x: 10, y: 12};
  const closer = {...creature, subject_entity_index: 11, x: 40, y: 30};
  const pickup = {...creature, subject_entity_index: 12, help_selector: 20, x: 80, y: 50};
  const helpState = {...visible, world_map: {entities: [creature, closer, pickup], marks: []}};
  const context = {generation: 5, sequence: 0, enabled: true, paused: false, suppressed: false,
    player: {x: 0, y: 0}, entities: [{entity: 10, x: 300, y: 0}, {entity: 11, x: 100, y: 0}, {entity: 12, x: 50, y: 0}]};
  const advance = (sequence: number, next = helpState) => dayHud.advanceMapHints(next, {...context, sequence});
  advance(1); advance(2);
  assert.equal(dayHud.visibleMapHint(), null, 'selection is drawn on the following source frame');
  advance(3);
  assert.deepEqual(dayHud.visibleMapHint(), {row: 19, subject: 11, x: 80, y: 60}, 'raw world distance and source row order select the creature');
  const shown = dayHud.compose(scene, palette, helpState);
  assert.notDeepEqual(shown, baseline, 'automatic source hint draws its frame and pointer');
  const movedHelp = {...helpState, world_map: {entities: [creature, {...closer, x: 44}, pickup], marks: []}};
  advance(4, movedHelp);
  assert.equal(dayHud.visibleMapHint()?.x, 88, 'the leader follows its original subject');
  advance(5, {...helpState, world_map: {entities: [creature, pickup], marks: []}});
  assert.equal(dayHud.visibleMapHint(), null, 'disappearance closes the live pointer');
  for (let sequence = 6; sequence <= 25; sequence++) advance(sequence, {...helpState, world_map: {entities: [pickup], marks: []}});
  assert.equal(dayHud.visibleMapHint(), null, 'closing waits 20 polls and selects without drawing');
  advance(26, {...helpState, world_map: {entities: [pickup], marks: []}});
  assert.equal(dayHud.visibleMapHint()?.row, 20, 'pickup hint appears after the source delay');
  advance(27, {...helpState, world_map: {entities: [], marks: []}});
  assert.equal(dayHud.visibleMapHint(), null, 'pickup collection clears its pointer');
  const onlyPickup = {...helpState, world_map: {entities: [pickup], marks: []}};
  const nextEpoch = {...context, generation: 6};
  for (let sequence = 1; sequence <= 2; sequence++) dayHud.advanceMapHints(onlyPickup, {...nextEpoch, sequence});
  let visibleFrames = 0;
  for (let sequence = 3; sequence <= 52; sequence++) {
    dayHud.advanceMapHints(onlyPickup, {...nextEpoch, sequence});
    if (dayHud.visibleMapHint()?.row === 20) visibleFrames++;
  }
  assert.equal(visibleFrames, 50, 'selected row owns its full 50 source presentation frames');
  dayHud.compose(scene, palette, onlyPickup);
  dayHud.compose(scene, palette, onlyPickup);
  assert.equal(dayHud.visibleMapHint()?.row, 20, 'repaint does not advance the hint clock');
  dayHud.advanceMapHints(onlyPickup, {...nextEpoch, sequence: 53});
  assert.equal(dayHud.visibleMapHint(), null);
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
