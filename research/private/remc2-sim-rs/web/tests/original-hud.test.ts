import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
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

test('spell improvement uses the owner text and lasts 200 admitted draws', () => {
  const scene = new Uint8Array(64000).fill(7), palette = new Uint8Array(768);
  const event = { generation: '77', sequence: '17', kind: 0x101, code: 5, value0: 1, value1: 0 };
  const advance = (sequence: number, suppressed = false) => hud.advanceAbilityNotification({
    generation: '77', sequence, assetRevision: 7, effects: [event], suppressed,
  });
  const plain = hud.compose(scene, palette, state);
  advance(1);
  assert.equal(hud.abilityNotificationText(), 'Your ability to cast FIREBALL has improved.');
  assert.notDeepEqual(hud.compose(scene, palette, state), plain);
  // A duplicate publication or a repaint never consumes a draw.
  for (let i = 0; i < 10; i++) { advance(1); hud.compose(scene, palette, state); }
  hud.advanceAbilityNotification({ generation: '78', sequence: 1, assetRevision: 7, effects: [event], suppressed: true });
  assert.equal(hud.abilityNotificationText(), null, 'pause-generation changes preserve the held notification');
  advance(2, true); assert.equal(hud.abilityNotificationText(), null);
  for (let sequence = 3; sequence <= 201; sequence++) advance(sequence);
  assert.equal(hud.abilityNotificationText(), 'Your ability to cast FIREBALL has improved.');
  advance(202); assert.equal(hud.abilityNotificationText(), null);
  assert.deepEqual(hud.compose(scene, palette, state), plain);
  hud.advanceAbilityNotification({ generation: '78', sequence: 203, assetRevision: 8, effects: [], suppressed: false });
  assert.equal(hud.abilityNotificationText(), null, 'a new bound mission clears the notification');
});

test('native death notice draws in the death view through its final source frame', () => {
  const scene = new Uint8Array(64000).fill(7), palette = new Uint8Array(768);
  const base = { ...state, death_view: true };
  const plain = hud.compose(scene, palette, base);
  const notice = { is_local: true, display_name: 'PLAYER', kind: 'participant.died', remaining_frames: 0 };
  const visible = hud.compose(scene, palette, { ...base, participant_notifications: [notice] });
  assert.notDeepEqual(visible, plain);
  const ownerColor = assets.manifest.ability_notification.color;
  assert.ok(visible.subarray(6 * 640 + 6, 20 * 640).includes(ownerColor), 'death text uses source font/color in the source death-view region');
  assert.deepEqual(hud.compose(scene, palette, { ...base, participant_notifications: [] }), plain);
  assert.equal(assets.manifest.participant_notices['participant.died'], 'has died.');
});

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

test('native hand-selection counters draw only their selected map hand and clear at zero', () => {
  const scene = new Uint8Array(64000).fill(7), palette = new Uint8Array(768);
  const map: OriginalHudState = {...state, top_bar_visible: false, world_map: {entities: [], marks: []}};
  const idle = hud.compose(scene, palette, {...map, hand_selection_flash_ticks: [0, 0]});
  const left = hud.compose(scene, palette, {...map, hand_selection_flash_ticks: [8, 0]});
  const right = hud.compose(scene, palette, {...map, hand_selection_flash_ticks: [0, 1]});
  const countChanges = (image: Uint8Array, start: number, end: number) => {
    let count = 0;
    for (let y = 0; y < 48; y++) for (let x = start; x < end; x++)
      if (image[y * 640 + x] !== idle[y * 640 + x]) count++;
    return count;
  };
  assert.ok(countChanges(left, 510, 574) > 0);
  assert.equal(countChanges(left, 574, 640), 0);
  assert.equal(countChanges(right, 510, 574), 0);
  assert.ok(countChanges(right, 574, 640) > 0);
  assert.deepEqual(hud.compose(scene, palette, {...map, hand_selection_flash_ticks: [0, 0]}), idle);
  const absent = {...map, hud: {...map.hud!, left: {...slot, holder_entity_index: 0}}};
  assert.deepEqual(hud.compose(scene, palette, {...absent, hand_selection_flash_ticks: [8, 0]}), idle);
  assert.throws(() => hud.compose(scene, palette, {...map, hand_selection_flash_ticks: [9, 0]}), /counters/);
});

test('player information draws live owner rows on the map and clears on release', () => {
  const scene = new Uint8Array(64000).fill(7), palette = new Uint8Array(768);
  const map = {...state, top_bar_visible: false, world_map: {entities: [], marks: []}};
  const plain = hud.compose(scene, palette, map);
  const board = { projection_error: null, players: [{source_slot: 0, display_name: 'Player',
    outline_palette_index: 41, text_palette_index: 42, max_mana: 123,
    kills8: [0, 0, 0, 0, 0, 0, 0, 0]}] };
  const visible = hud.compose(scene, palette, {...map, player_information: board});
  // One source row is centred using the 104x38 and 36x38 owner tiles.
  assert.equal(visible[225 * 640 + 288], 41);
  const withObjective = hud.compose(scene, palette, {...map, player_information: board, objective_visible: true, objective_selector: 48});
  assert.equal(withObjective[244 * 640 + 288], 41, 'scoreboard stays above the objective textbox');
  assert.ok(visible.includes(42), 'owner font draws the native name and mana in the player colour');
  assert.notDeepEqual(visible, plain);
  assert.deepEqual(hud.compose(scene, palette, {...map, player_information: null}), plain);
  assert.throws(() => hud.compose(scene, palette, {...map, player_information: {...board, projection_error: 'missing'}}), /unavailable/);
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

  assert.equal(qualified.manifest.help['22'].text, 'This stone restores your life');
  assert.equal(qualified.manifest.help['25'].text, 'This is an enemy castle');
  assert.equal(qualified.manifest.help['26'].text, 'This is an enemy balloon');
  const stone = {...creature, subject_entity_index: 21, help_selector: 22, x: 14, y: 18};
  const castle = {...creature, subject_entity_index: 22, help_selector: 25, x: 28, y: 32};
  const balloon = {...creature, subject_entity_index: 23, help_selector: 26, x: 36, y: 40};
  const subjects = {...context, generation: 8, entities: [
    {entity: 21, x: 40, y: 0}, {entity: 22, x: 100, y: 0}, {entity: 23, x: 20, y: 0},
  ]};
  const all = {...visible, world_map: {entities: [stone, castle, balloon], marks: []}};
  const mapAdvance = (sequence: number, next = all) => dayHud.advanceMapHints(next, {...subjects, sequence});
  mapAdvance(1); mapAdvance(2); mapAdvance(3);
  assert.deepEqual(dayHud.visibleMapHint(), {row: 25, subject: 22, x: 56, y: 64},
    'source priority and row order select the rival Castle before the nearer balloon or Dolmen');
  const castleMap = dayHud.compose(scene, palette, all);
  assert.notDeepEqual(castleMap, baseline, 'rival Castle help draws owner text');
  assert.equal(castleMap[Math.ceil(castle.y * 480 / 200) * 640 + castle.x * 2],
    qualified.manifest.help['25'].line_color, 'full-map leader reaches the expanded source marker');
  mapAdvance(4, {...all, world_map: {entities: [stone, {...castle, x: 30}, balloon], marks: []}});
  assert.equal(dayHud.visibleMapHint()?.x, 60, 'the leader tracks the live Castle marker');
  mapAdvance(5, {...all, world_map: {entities: [stone, balloon], marks: []}});
  assert.equal(dayHud.visibleMapHint(), null, 'Castle disappearance retires the pointer');
  for (let sequence = 6; sequence <= 26; sequence++) mapAdvance(sequence, {...all, world_map: {entities: [stone, balloon], marks: []}});
  assert.equal(dayHud.visibleMapHint()?.row, 26, 'remaining visible rival balloon wins the next poll');
  mapAdvance(27, {...all, world_map: {entities: [stone], marks: []}});
  assert.equal(dayHud.visibleMapHint(), null, 'a no-longer-visible balloon clears its pointer');
  for (let sequence = 28; sequence <= 48; sequence++) mapAdvance(sequence, {...all, world_map: {entities: [stone], marks: []}});
  assert.equal(dayHud.visibleMapHint()?.row, 22, 'healing Dolmen wins once higher-priority rows have gone');
  mapAdvance(49, {...all, world_map: {entities: [{...stone, visible: 0}], marks: []}});
  assert.equal(dayHud.visibleMapHint(), null, 'hidden Dolmen no longer presents guidance');

  const cornerCastle = {...castle, x: 64, y: 64};
  const corner = {...state, minimap: {terrain: Array(4096).fill(7), coverage: Array(4096).fill(1),
    entities: [cornerCastle], marks: []}};
  for (let sequence = 1; sequence <= 3; sequence++)
    dayHud.advanceMapHints(corner, {...subjects, generation: 9, sequence});
  assert.deepEqual(dayHud.visibleMapHint(), {row: 25, subject: 22, x: 64, y: 64});
  const cornerMap = dayHud.compose(scene, palette, corner);
  assert.equal(cornerMap[Math.ceil((cornerCastle.y / 2) * 480 / 200) * 640 + cornerCastle.x],
    qualified.manifest.help['25'].line_color, 'corner leader reaches the expanded source marker');

  assert.equal(qualified.manifest.help['27'].text, 'Marker stone');
  // These projections and blink phases were captured from the actual Jaleen
  // native marker rows; the single-subject pose below controls nearest selection.
  const recorded = JSON.parse(readFileSync(`${generated}original-marker-stone-20261122/candidate-real-entries.json`, 'utf8'))
    .filter((entry: {level: number}) => entry.level === 12);
  assert.equal(recorded.length, 12);
  const markerContext = {...context, generation: 10, entities: [{entity: 86, x: 35968, y: 28800}]};
  for (let i = 0; i < recorded.length; i++) {
    const entry = recorded[i];
    const sample = entry.full.find((row: number[]) => row[0] === 86);
    assert.ok(sample, 'the same authored Jaleen marker remains published');
    const actual = {...marker, tick: entry.tick, subject_entity_index: sample[0],
      x: sample[1], y: sample[2], visible: sample[3], help_selector: 27, kind: 1};
    const frame = {...visible, hud: {...visible.hud!, tick: entry.tick},
      world_map: {entities: [actual], marks: []}};
    dayHud.advanceMapHints(frame, {...markerContext, sequence: i + 1});
    if (i >= 2) {
      assert.deepEqual(dayHud.visibleMapHint(), {row: 27, subject: 86, x: 222, y: 270},
        'the live marker pointer survives both phases of the recorded blink');
      const pixels = dayHud.compose(scene, palette, frame);
      assert.equal(pixels[Math.ceil(actual.y * 480 / 200) * 640 + actual.x * 2],
        qualified.manifest.help['27'].line_color, 'the leader reaches the actual full-map marker');
    }
  }
  const outside = {...marker, tick: 11, subject_entity_index: 86, help_selector: 27, kind: 1, x: 191, y: 135};
  dayHud.advanceMapHints({...visible, hud: {...visible.hud!, tick: 11}, world_map: {entities: [outside], marks: []}},
    {...markerContext, sequence: 13});
  assert.equal(dayHud.visibleMapHint(), null, 'a marker outside the source map clears its pointer');

  for (let i = 0; i < recorded.length; i++) {
    const entry = recorded[i];
    const sample = entry.corner.find((row: number[]) => row[0] === 86);
    const actual = {...marker, tick: entry.tick, subject_entity_index: sample[0],
      x: sample[1], y: sample[2], visible: sample[3], help_selector: 27, kind: 1};
    const frame = {...corner, hud: {...corner.hud!, tick: entry.tick},
      minimap: {...corner.minimap, entities: [actual]}};
    dayHud.advanceMapHints(frame, {...markerContext, generation: 11, sequence: i + 1});
    if (i >= 2) {
      assert.deepEqual(dayHud.visibleMapHint(), {row: 27, subject: 86, x: 90, y: 120},
        'the corner-map pointer also survives the distinct blink cadence');
      const pixels = dayHud.compose(scene, palette, frame);
      assert.equal(pixels[Math.ceil(60 * 480 / 200) * 640 + 90],
        qualified.manifest.help['27'].line_color, 'the corner leader reaches the source marker');
    }
  }
  const outsideCorner = {...outside, x: 0, y: 0, visible: 0};
  dayHud.advanceMapHints({...corner, hud: {...corner.hud!, tick: 11},
    minimap: {...corner.minimap, entities: [outsideCorner]}}, {...markerContext, generation: 11, sequence: 13});
  assert.equal(dayHud.visibleMapHint(), null, 'leaving the corner aperture retires the pointer');
  const wrongKind = {...outside, x: 111, y: 135, visible: 0, kind: 2};
  const wrongFrame = {...visible, hud: {...visible.hud!, tick: 11}, world_map: {entities: [wrongKind], marks: []}};
  for (let sequence = 1; sequence <= 3; sequence++)
    dayHud.advanceMapHints(wrongFrame, {...markerContext, generation: 12, sequence});
  assert.equal(dayHud.visibleMapHint(), null, 'the blink exception does not admit another hidden entity kind');

  // Exercise the common poll/lifetime carrier with the actual owner strings;
  // the Castle status is held fixed to inspect every lower-priority row.
  const status = {castle_rows: [{stage: 1, panel_selector: 41, icon_selector: 43,
    life: 40, max_life: 100, mana: 5, max_mana: 50, owner_mana_aggregate: 0}],
    balloon_panel_selector: 41, balloon_icon_selector: 43,
    balloons: Array.from({length: 3}, () => ({present: true, life: 20, max_life: 100, mana: 0, max_mana: 50})),
    mana_goal_percent: 40, goal_palette_index: 0};
  const statusFrame = {...state, source_top_status: status, castle_goal_hint_eligible: true,
    minimap: {...corner.minimap, entities: []}};
  const statusContext = {...context, generation: 13, entities: []};
  const expected = [[29,214,32],[30,214,16],[31,130,16],[32,320,32],[33,260,16],[34,320,16],[35,89,38]];
  const drawn = new Map<number, number>();
  for (let sequence = 1; sequence <= 520; sequence++) {
    dayHud.advanceMapHints(statusFrame, {...statusContext, sequence});
    const hint = dayHud.visibleMapHint();
    if (!hint) continue;
    const row = expected.find(value => value[0] === hint.row);
    assert.ok(row);
    assert.deepEqual([hint.x, hint.y], row.slice(1));
    drawn.set(hint.row, (drawn.get(hint.row) ?? 0) + 1);
    const pixels = dayHud.compose(scene, palette, statusFrame);
    assert.equal(pixels[hint.y * 640 + hint.x], qualified.manifest.help[String(hint.row)].line_color);
  }
  assert.deepEqual([...drawn], expected.map(row => [row[0], 50]), 'each eligible source row draws for exactly 50 frames');
  const ineligible = {...statusFrame, castle_goal_hint_eligible: false};
  let ordinaryRows = 0;
  for (let sequence = 1; sequence <= 700; sequence++) {
    dayHud.advanceMapHints(ineligible, {...statusContext, generation: 14, sequence});
    const hint = dayHud.visibleMapHint();
    if (hint) { assert.notEqual(hint.row, 35, 'a percentage alone does not qualify row 35'); ordinaryRows++; }
  }
  assert.ok(ordinaryRows > 0, 'the other Castle and balloon rows remain available');
  const openMap = {...ineligible, world_map: {entities: [], marks: []}};
  for (let sequence = 1; sequence <= 100; sequence++) {
    dayHud.advanceMapHints(openMap, {...statusContext, generation: 15, sequence});
    assert.equal(dayHud.visibleMapHint(), null, 'status help is not selected over the full map');
  }

  assert.equal(qualified.manifest.help['18'].text, 'Possess This Mana');
  // Real Day 3 replay at tick 2431: slot 259 is an unclaimed 500-mana
  // sphere. Hold its native projection and pose fixed to test selection.
  const mana = {...marker, generation: 3, tick: 2431, subject_entity_index: 259,
    help_selector: 18, kind: 1, selector: 0, palette_index: 232, x: 89, y: 89, visible: 1};
  const other = {...creature, generation: 3, tick: 2431, subject_entity_index: 10};
  const manaFrame = {...visible, hud: {...visible.hud!, generation: 3, tick: 2431},
    world_map: {entities: [mana, other], marks: []}};
  const manaContext = {...context, generation: 16, player: {x: 22629, y: 53300},
    entities: [{entity: 259, x: 22629, y: 53300}, {entity: 10, x: 22629, y: 53300}]};
  for (let sequence = 1; sequence <= 3; sequence++) dayHud.advanceMapHints(manaFrame, {...manaContext, sequence});
  assert.deepEqual(dayHud.visibleMapHint(), {row: 18, subject: 259, x: 178, y: 178},
    'source order selects the live loose-mana row before an equal-priority creature');
  const manaPixels = dayHud.compose(scene, palette, manaFrame);
  assert.equal(manaPixels[Math.ceil(89 * 480 / 200) * 640 + 178], qualified.manifest.help['18'].line_color);
  const claimed = {...mana, help_selector: 0};
  dayHud.advanceMapHints({...manaFrame, world_map: {entities: [claimed], marks: []}}, {...manaContext, sequence: 4});
  assert.equal(dayHud.visibleMapHint(), null, 'claiming removes row18 even while its owned map dot remains');
  let manaDraws = 0;
  for (let sequence = 1; sequence <= 53; sequence++) {
    dayHud.advanceMapHints({...manaFrame, world_map: {entities: [mana], marks: []}}, {...manaContext, generation: 17, sequence});
    if (dayHud.visibleMapHint()?.row === 18) manaDraws++;
  }
  assert.equal(manaDraws, 50, 'loose-mana help expires after 50 source draws');
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
