import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { decodeRenderPublication } from '../src/render-publication.ts';

// Hand-authored v5 packet: every nested field, each option, non-ASCII UTF-8,
// signed values, enum cases and identifiers beyond JavaScript's safe integers.
function packet(emptyOptions = false, announcement: number | null = emptyOptions ? null : 284) {
  const bytes: number[] = [];
  const offsets: Record<string, number> = {};
  const mark = (name: string) => { offsets[name] = bytes.length; };
  const u8 = (v: number) => bytes.push(v & 255);
  const u16 = (v: number) => { u8(v); u8(v >>> 8); };
  const u32 = (v: number) => { u16(v); u16(v >>> 16); };
  const u64 = (v: bigint) => { for (let i = 0n; i < 8n; i++) u8(Number((v >> (8n * i)) & 255n)); };
  const str = (s: string) => { const b = new TextEncoder().encode(s); u32(b.length); bytes.push(...b); };
  bytes.push(77, 67, 80, 79); u32(5); u32(0);
  u64(9007199254740993n); u64(9007199254740994n); mark('mode'); u8(1);
  mark('firstStringLength'); str('é'); str('graph'); str('definition');
  u64(2n); u64(3n); u64(4n); u64(5n); u64(6n); mark('valueTick'); u64(5n); u64(9007199254740995n);
  [1, 2, -3, -4, -5, -6, -7].forEach(u16);
  [-8, -9, -10].forEach(u16);
  [-11, 12, -13, 14].forEach(u32);
  mark('menuFlag'); u8(emptyOptions ? 0 : 1);
  if (!emptyOptions) { u8(8); u8(1); u8(0); u8(1); u8(0); u64(5n); }
  mark('returnBool'); u8(1);
  mark('playersCount'); u32(1); u8(9); str('Player'); u8(10); u8(11); u32(-12);
  [-1, 2, -3, 4, -5, 6, -7, 8].forEach(u16);
  u8(emptyOptions ? 0 : 1); if (!emptyOptions) str('projection failure');
  u32(1); u64(9007199254740996n); u8(1); str('Mág'); str('notice.spell'); u16(-9);
  u8(-12); u16(15); [16, 17, -18, -19, -20, -21].forEach(u16); u16(-22);
  u8(1); u8(45); u8(47); u8(0);
  u32(1); str('spell.fireball'); str('Fireball'); u8(1); u8(emptyOptions ? 0 : 1);
  if (!emptyOptions) u64(9007199254740997n);
  u8(1); u8(-3); u8(4); u8(5); u8(emptyOptions ? 0 : 1);
  if (!emptyOptions) { u16(6); [7, 8, 9].forEach(u16); }
  u8(emptyOptions ? 0 : 1);
  if (!emptyOptions) {
    mark('helpStandard'); u8(1); mark('helpTier'); u8(2);
    mark('helpRecord'); [186, 187, 188].forEach(u16);
    [0, 1, 0, 1].forEach(u8);
    [1, 0, 1].forEach(u8); [-10, 20, 30, 17, 100, 200, 300].forEach(u32);
  }
  u32(-10); u8(3); u8(2); u8(5);
  u8(emptyOptions ? 0 : 1); if (!emptyOptions) str('objective.finish');
  u32(17); u32(18); u8(1); u8(19); u16(announcement ?? 0); u8(1);
  u32(1); u64(9007199254740998n); str('creature.goat'); [20, 21, -22, -23, -24].forEach(u16);
  u16(25); str('asset.goat'); str('sha256:abcd');
  mark('flashLeft'); u8(8); mark('flashRight'); u8(1);
  mark('castlePresent'); u8(emptyOptions ? 0 : 1);
  if (!emptyOptions) {
    mark('castleStage'); u8(0); u16(41); u16(43);
    [-1, 200, -3, 400, -5].forEach(u32);
  }
  u16(41); u16(50);
  mark('balloon0'); u8(1); [-2, 120, -7, 90].forEach(u32);
  mark('balloon1'); u8(0); [0, 0, 0, 0].forEach(u32);
  mark('balloon2'); u8(1); [75, 100, 33, 200].forEach(u32);
  u16(-11); u8(123);
  const result = Uint8Array.from(bytes);
  new DataView(result.buffer).setUint32(8, result.length, true);
  return { result, offsets };
}

test('decodes all v5 fields, preserves wide integers, and owns an immutable result', () => {
  const { result } = packet();
  const actual = decodeRenderPublication(result);
  const padded = new Uint8Array(result.length + 2); padded.set(result, 1);
  assert.deepEqual(decodeRenderPublication(padded.subarray(1, -1)), actual);
  assert.deepEqual(actual, {
    runtime_id: 9007199254740993n, runtime_generation: 9007199254740994n, runtime_mode: 'paused',
    identity: { engine_contract: 'é', package_graph_hash: 'graph', definition_hash: 'definition',
      definition_revision: 2n, session_id: 3n, generation: 4n, tick: 5n },
    terrain_mutation_sequence: 6n,
    value: { tick: 5n, effect_head: 9007199254740995n,
      player_pose: { x: 1, y: 2, z: -3, yaw: -4, display_pitch: -5, effective_pitch: -6, roll: -7 },
      player_motion: { target_speed: -8, actual_speed: -9, strafe_speed: -10 },
      player_vitals: { life: -11, max_life: 12, mana: -13, max_mana: 14 },
      source_menu_admission: { action: 'close_gameplay_menus', admitted: true, map_open: false,
        spells_open: true, player_information_open: false, tick: 5n }, return_to_title_requested: true,
      source_scoreboard: { players: [{ source_slot: 9, display_name: 'Player', outline_palette_index: 10,
        text_palette_index: 11, max_mana: -12, kills8: [-1, 2, -3, 4, -5, 6, -7, 8] }], projection_error: 'projection failure' },
      participant_notifications: [{ participant: 9007199254740996n, is_local: true, display_name: 'Mág', kind: 'notice.spell', remaining_frames: -9 }],
      player_wall_contact_counter: -12, player_immobilized_ticks: 15,
      source_camera_pose: { x: 16, y: 17, z: -18, yaw: -19, pitch: -20, roll: -21 }, source_camera_fov: -22,
      spell_menu_has_any_spell: true, spell_menu_wizard_colors: [45, 47], spell_menu_panels_opaque: false,
      player_spells: [{ id: 'spell.fireball', display_name: 'Fireball', acquired: true, holder_actor: 9007199254740997n,
        selected: true, tier: -3, unlocked_tier: 4, menu_slot: 5, menu_artwork: { grid_icon: 6, tier_icons: [7, 8, 9] },
        menu_help: { is_standard: true, selected_tier: 2, title_records: [186, 187, 188],
          castle_blocked: [false, true, false], mana_unavailable: true,
          definition_enabled: true, blink_hidden: false, discovered: true,
          tier_mana_costs: [-10, 20, 30], experience: 17, experience_thresholds: [100, 200, 300] },
        mana_cost: -10, hand: 'both', marked_hand: 'right', availability: 'insufficient_mana' }],
      objective_progress: { current_objective: 'objective.finish', completed_count: 17, total_count: 18, all_objectives_satisfied: true },
      objective_textbox_ticks: 19, objective_secret_announcement_record: 284, exit_sequence_active: true,
      player_hand_selection_flash_ticks: [8, 1],
      source_top_status: { castle_rows: [{ stage: 0, panel_selector: 41, icon_selector: 43,
        life: -1, max_life: 200, mana: -3, max_mana: 400, owner_mana_aggregate: -5 }],
        balloon_panel_selector: 41, balloon_icon_selector: 50, balloons: [
          { present: true, life: -2, max_life: 120, mana: -7, max_mana: 90 },
          { present: false, life: 0, max_life: 0, mana: 0, max_mana: 0 },
          { present: true, life: 75, max_life: 100, mana: 33, max_mana: 200 }],
        mana_goal_percent: -11, goal_palette_index: 123 },
      presentation_entities: [{ actor: 9007199254740998n, kind: 'creature.goat', pose: { x: 20, y: 21, z: -22, yaw: -23, pitch: -24 },
        logical_height: 25, asset: { id: 'asset.goat', content_address: 'sha256:abcd' } }] },
  });
  result.fill(0);
  assert.equal(actual.identity.engine_contract, 'é');
  assert.ok(Object.isFrozen(actual.value.player_spells[0]!.menu_artwork!.tier_icons));
  assert.ok(Object.isFrozen(actual.value.player_spells[0]!.menu_help!.castle_blocked));
  assert.throws(() => { (actual.value.player_spells as unknown as unknown[]).push(null); }, TypeError);
});

test('represents absent options consistently with the diagnostic projection', () => {
  const actual = decodeRenderPublication(packet(true).result).value;
  assert.ok(!('source_menu_admission' in actual));
  assert.ok(!('holder_actor' in actual.player_spells[0]!));
  assert.ok(!('menu_artwork' in actual.player_spells[0]!));
  assert.ok(!('menu_help' in actual.player_spells[0]!));
  assert.equal(actual.source_scoreboard.projection_error, null);
  assert.equal(actual.objective_progress.current_objective, null);
  assert.equal(actual.objective_secret_announcement_record, null);
  assert.deepEqual(actual.source_top_status.castle_rows, []);
});

test('carries the selected special text record and rejects unbound records', () => {
  for (const record of [284, 285])
    assert.equal(decodeRenderPublication(packet(false, record).result).value.objective_secret_announcement_record, record);
  assert.throws(() => decodeRenderPublication(packet(false, 283).result), /announcement/);
  assert.throws(() => decodeRenderPublication(packet(false, 286).result), /announcement/);
});

test('rejects invalid headers, truncation, booleans, UTF-8, counts and trailing bytes', () => {
  const { result, offsets } = packet();
  for (let length = 0; length < result.length; length++) {
    const shortened = result.slice(0, length);
    if (length >= 12) new DataView(shortened.buffer).setUint32(8, length, true);
    assert.throws(() => decodeRenderPublication(shortened), `truncation at ${length}`);
  }
  const changed = (offset: number, value: number) => { const b = result.slice(); b[offset] = value; return b; };
  assert.throws(() => decodeRenderPublication(changed(0, 0)), /magic/);
  assert.throws(() => decodeRenderPublication(changed(4, 1)), /version/);
  assert.throws(() => decodeRenderPublication(changed(4, 2)), /version/);
  assert.throws(() => decodeRenderPublication(changed(4, 3)), /version/);
  assert.throws(() => decodeRenderPublication(changed(4, 4)), /version/);
  assert.throws(() => decodeRenderPublication(changed(8, 0)), /length/);
  assert.throws(() => decodeRenderPublication(changed(offsets.mode!, 2)), /enum/);
  assert.throws(() => decodeRenderPublication(changed(offsets.valueTick!, 6)), /tick mismatch/);
  assert.throws(() => decodeRenderPublication(changed(offsets.menuFlag!, 2)), /boolean/);
  assert.throws(() => decodeRenderPublication(changed(offsets.returnBool!, 2)), /boolean/);
  assert.throws(() => decodeRenderPublication(changed(offsets.helpStandard!, 2)), /boolean/);
  assert.throws(() => decodeRenderPublication(changed(offsets.helpTier!, 3)), /spell menu help tier/);
  assert.throws(() => decodeRenderPublication(changed(offsets.flashLeft!, 9)), /hand selection/);
  assert.throws(() => decodeRenderPublication(changed(offsets.flashRight!, 9)), /hand selection/);
  assert.throws(() => decodeRenderPublication(changed(offsets.castlePresent!, 2)), /boolean/);
  assert.throws(() => decodeRenderPublication(changed(offsets.castleStage!, 8)), /Castle stage/);
  for (const key of ['balloon0', 'balloon1', 'balloon2'])
    assert.throws(() => decodeRenderPublication(changed(offsets[key]!, 2)), /boolean/);
  assert.deepEqual(decodeRenderPublication(changed(offsets.flashRight!, 0)).value.player_hand_selection_flash_ticks, [8, 0]);
  const badTitle = result.slice();
  new DataView(badTitle.buffer).setUint16(offsets.helpRecord!, 471, true);
  assert.throws(() => decodeRenderPublication(badTitle), /spell menu help tier or language record/);
  assert.throws(() => decodeRenderPublication(changed(offsets.firstStringLength! + 4, 0xff)), /encoded data|encoding|UTF-8/i);
  const oversizedString = result.slice(); new DataView(oversizedString.buffer).setUint32(offsets.firstStringLength!, 0xffffffff, true);
  assert.throws(() => decodeRenderPublication(oversizedString), /string exceeds/);
  const oversizedArray = result.slice(); new DataView(oversizedArray.buffer).setUint32(offsets.playersCount!, 0xffffffff, true);
  assert.throws(() => decodeRenderPublication(oversizedArray), /array exceeds/);
  const extra = new Uint8Array(result.length + 1); extra.set(result); new DataView(extra.buffer).setUint32(8, extra.length, true);
  assert.throws(() => decodeRenderPublication(extra), /trailing/);
});

// This deliberately requires a real, explicitly supplied library and owner packages.
// Example: MC_RENDER_NATIVE_LIB=/path/libsim_capi.so node --test tests/render-publication.test.ts
// The Python driver supplies a default locally ignored scenario, overridable via CLI/environment.
test('native C ABI publication agrees field-for-field with its diagnostic projection', { skip: !process.env.MC_RENDER_NATIVE_LIB }, () => {
  const run = spawnSync('python3', ['tests/render-publication-native.py', '--library', process.env.MC_RENDER_NATIVE_LIB!], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  assert.equal(run.status, 0, run.stderr);
  const cases = JSON.parse(run.stdout) as { binary: string; diagnostic: Record<string, unknown>; runtime: { runtime_id: string; generation: string; mode: string } }[];
  assert.ok(cases.length >= 2);
  for (const { binary, diagnostic, runtime } of cases) {
    const bytes = Buffer.from(binary, 'base64');
    const decoded = decodeRenderPublication(bytes);
    assert.equal(decoded.runtime_id.toString(), runtime.runtime_id);
    assert.equal(decoded.runtime_generation.toString(), runtime.generation);
    assert.equal(decoded.runtime_mode, runtime.mode);
    const actual = JSON.parse(JSON.stringify({ identity: decoded.identity, terrain_mutation_sequence: decoded.terrain_mutation_sequence, value: decoded.value },
      (_key, value) => typeof value === 'bigint' ? value.toString() : value));
    const { contract: _contract, ...expected } = diagnostic;
    assert.deepEqual(actual, expected);
    const malformed = Uint8Array.from(bytes); malformed[4] = 2;
    assert.throws(() => decodeRenderPublication(malformed), /version/);
    assert.throws(() => decodeRenderPublication(bytes.subarray(0, bytes.length - 1)), /length/);
  }
});
