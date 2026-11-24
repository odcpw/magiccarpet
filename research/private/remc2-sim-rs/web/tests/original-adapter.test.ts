import { test } from 'node:test';
import assert from 'node:assert/strict';
import { decodeOriginalFrame, ORIGINAL_PACKET_BYTES, OriginalSceneHistory, originalInput, markedSpellCycle, markedSpellHand, preferenceKeyCode, sourceMenuKeys, sourceMapKey, sourceKeyChoices, mapGestureAdmitted, MapMouseChord } from '../src/original-adapter.ts';

test('map binding translates every native preference key to a physical browser code', () => {
  for (const letter of 'ABCDEFGHIJKLMNOQSTUVWXYZ') assert.equal(preferenceKeyCode(letter), `Key${letter}`);
  const named = { Minus: 'Minus', Equals: 'Equal', Semicolon: 'Semicolon', Backslash: 'Backslash',
    Comma: 'Comma', Period: 'Period', Slash: 'Slash', UpArrow: 'ArrowUp', DownArrow: 'ArrowDown',
    LeftArrow: 'ArrowLeft', RightArrow: 'ArrowRight', Return: 'Enter', Space: 'Space',
    LeftControl: 'ControlLeft', RightControl: 'ControlRight', LeftShift: 'ShiftLeft',
    RightShift: 'ShiftRight', LeftAlt: 'AltLeft', RightAlt: 'AltRight' };
  for (const [key, code] of Object.entries(named)) assert.equal(preferenceKeyCode(key), code);
  for (const invalid of [null, undefined, '', 'Tab', 'P', '__proto__']) assert.equal(preferenceKeyCode(invalid), null);
});

test('source menu bindings and hardcoded left modifiers follow held and released keys', () => {
  const bindings = ['UpArrow', 'DownArrow', 'LeftArrow', 'RightArrow', 'B', 'J', 'K', 'L', 'M', 'N'].map(preferenceKeyCode);
  const read = (...codes: string[]) => sourceMenuKeys(new Set(codes), bindings);
  assert.deepEqual(read(), { spells: false, information: false, mark: false, next: false, previous: false });
  assert.equal(read('KeyJ').spells, true);
  assert.equal(read('ControlLeft').spells, false, 'superseded spell binding is inactive');
  assert.equal(read('KeyK').information, true);
  assert.equal(read('KeyL').mark, true); assert.equal(read('KeyL').next, false);
  assert.equal(read('KeyM').next, true); assert.equal(read('KeyN').previous, true);
  assert.equal(read('KeyN').information, true, 'previous modifier also reveals map information');
  assert.equal(read('ShiftLeft').mark, true); assert.equal(read('ShiftLeft').next, true);
  assert.equal(read('AltLeft').previous, true); assert.equal(read('AltLeft').information, true);
  assert.equal(read('ShiftRight').mark, false); assert.equal(read('AltRight').previous, false);
  const held = new Set(['KeyK', 'KeyN']);
  held.delete('KeyK');
  assert.equal(sourceMenuKeys(held, bindings).information, true, 'previous remains held after info key release');
  held.delete('KeyN');
  assert.equal(sourceMenuKeys(held, bindings).information, false, 'last release hides information');
  held.add('KeyJ'); held.clear();
  assert.equal(sourceMenuKeys(held, bindings).spells, false, 'focus clear does not retain a menu modifier');
  assert.equal(sourceMapKey('Tab', 'Enter'), true);
  assert.equal(sourceMapKey('Tab', 'KeyB'), false);
  assert.equal(sourceMapKey('KeyB', 'KeyB'), true);
  assert.equal(sourceKeyChoices('LeftAlt', 'LeftAlt', 'LeftAlt'), 'LeftAlt');
  assert.equal(sourceKeyChoices('LeftAlt', 'K', 'N'), 'LeftAlt or K or N');
});

test('map chord uses a physical edge, consumes both buttons, and respects menu precedence', () => {
  const flight = { mapOpen: false, spellsOpen: false, controlHeld: false, cycleHeld: false };
  const chord = new MapMouseChord();
  assert.equal(chord.press(0, flight), false);
  assert.equal(chord.press(2, flight), true);
  assert.equal(chord.isConsumed(0), true); assert.equal(chord.isConsumed(2), true);
  assert.equal(chord.press(2, flight), false, 'held buttons do not toggle twice');
  chord.release(2); assert.equal(chord.isConsumed(2), false); assert.equal(chord.isConsumed(0), true);
  assert.equal(chord.press(2, { ...flight, mapOpen: true }), true, 'a fresh edge closes the map');
  chord.clear();
  assert.equal(chord.press(2, flight), false);
  assert.equal(chord.press(0, flight), true, 'right then left also opens the map');
  chord.clear();
  assert.equal(chord.press(0, { ...flight, controlHeld: true }), false);
  assert.equal(chord.press(2, { ...flight, controlHeld: true }), false, 'Ctrl wins in flight');
  assert.equal(mapGestureAdmitted({ ...flight, cycleHeld: true }), false, 'modified spell cycling wins in flight');
  assert.equal(mapGestureAdmitted({ ...flight, mapOpen: true, controlHeld: true }), true, 'map close precedes Ctrl');
  assert.equal(mapGestureAdmitted({ ...flight, mapOpen: true, cycleHeld: true }), true, 'map close precedes cycling');
  assert.equal(mapGestureAdmitted({ ...flight, mapOpen: true, spellsOpen: true }), false, 'open spell menu wins');
});

test('source marked-spell gestures choose the clicked hand and reverse modifier', () => {
  assert.deepEqual(markedSpellCycle(0, true, false), {hand: 'left', direction: 'next'});
  assert.deepEqual(markedSpellCycle(2, true, true), {hand: 'right', direction: 'previous'});
  assert.deepEqual(markedSpellCycle(0, false, true), {hand: 'left', direction: 'previous'});
  assert.equal(markedSpellCycle(0, false, false), null);
  assert.equal(markedSpellCycle(1, true, false), null);
  assert.equal(markedSpellHand(0, 'none'), 'left');
  assert.equal(markedSpellHand(0, 'left'), 'none');
  assert.equal(markedSpellHand(2, 'left'), 'right');
  assert.equal(markedSpellHand(2, 'right'), 'none');
});

function packet(revision = 1n, flags = 1, generation = 5n): Uint8Array {
  const bytes = new Uint8Array(ORIGINAL_PACKET_BYTES); const v = new DataView(bytes.buffer);
  v.setUint32(0, 0x3146434d, true); v.setUint16(4, 1, true); v.setUint16(6, 56, true);
  v.setUint16(8, 320, true); v.setUint16(10, 200, true); v.setUint32(12, flags, true);
  v.setBigUint64(16, generation, true); v.setBigUint64(24, 6n, true);
  v.setBigUint64(32, 9007199254740993n, true); v.setBigUint64(40, revision, true); v.setBigUint64(48, revision, true);
  bytes.fill(7, 56, 56 + 64000); bytes[56 + 64000] = 123;
  return bytes;
}

test('bounded versioned frame preserves u64 and owns copied pixels/palette, including offset views', () => {
  const source = packet(); const prefixed = new Uint8Array(source.length + 17); prefixed.set(source, 9);
  const frame = decodeOriginalFrame(prefixed.subarray(9, 9 + source.length));
  source.fill(0); prefixed.fill(0);
  assert.equal(frame.tick, 9007199254740993n); assert.equal(frame.indices.length, 64000);
  assert.equal(frame.indices[63999], 7); assert.equal(frame.palette[0], 123); assert.equal(frame.palette.length, 768);
});

test('reject truncated, oversized, wrong-version, wrong-dimension and incomplete source packets', () => {
  for (const size of [0, 55, ORIGINAL_PACKET_BYTES - 1, ORIGINAL_PACKET_BYTES + 1])
    assert.throws(() => decodeOriginalFrame(new Uint8Array(size)));
  for (const [offset, value] of [[0, 0], [4, 2], [6, 48], [8, 640], [10, 400], [12, 0], [12, 3], [12, 7], [12, 17]]) {
    const bytes = packet(); new DataView(bytes.buffer).setUint16(offset!, value!, true);
    assert.throws(() => decodeOriginalFrame(bytes));
  }
});

test('history blends current-high/prior-low once per revision and clears across generations', () => {
  const history = new OriginalSceneHistory(); const normal = new Uint8Array(65536), exit = new Uint8Array(65536);
  normal[(9 << 8) | 7] = 41; exit[(10 << 8) | 41] = 99;
  assert.equal(history.compose(decodeOriginalFrame(packet()))[0], 7);
  const next = packet(2n, 5); next[56] = 9;
  assert.equal(history.compose(decodeOriginalFrame(next), { normal, exit })[0], 41);
  assert.equal(history.compose(decodeOriginalFrame(next), { normal, exit })[0], 41, 'same revision never compounds history');
  const ending = packet(3n, 13); ending[56] = 10;
  assert.equal(history.compose(decodeOriginalFrame(ending), { normal, exit })[0], 99);
  assert.equal(history.compose(decodeOriginalFrame(packet(4n, 5, 12n)))[0], 7, 'load/reset must not retain old-scene ghosts');
  assert.throws(() => history.compose(decodeOriginalFrame(packet(5n, 5, 12n))), /history table/);
});

test('browser input is bounded native intent and clearing keys releases movement/fire', () => {
  const held = originalInput(new Set(['KeyW', 'KeyA', 'ArrowDown']), Infinity, 999, { selected: true });
  assert.equal(held.longitudinal, 'accelerate'); assert.equal(held.lateral, 'negative');
  assert.equal(held.roll_axis, 0); assert.equal(held.pitch_axis, 64); assert.equal(held.cast_selected, true);
  const clear = originalInput(new Set()); assert.equal(clear.longitudinal, 'coast');
  assert.equal(clear.lateral, 'neutral'); assert.equal(clear.cast_selected, false);
});

test('native keyboard slots drive flight independently of mouse steering and touch', () => {
  const bound = ['UpArrow', 'DownArrow', 'LeftArrow', 'RightArrow', 'Return'].map(preferenceKeyCode);
  const input = (held: string[], keys = bound) => originalInput(new Set(held), 0, 0, {}, keys);
  const arrows = input(['ArrowUp', 'ArrowRight']);
  assert.equal(arrows.longitudinal, 'accelerate'); assert.equal(arrows.lateral, 'positive');
  assert.equal(arrows.pitch_axis, 0); assert.equal(arrows.roll_axis, 0);
  assert.equal(input(['ArrowLeft', 'ArrowRight']).lateral, 'positive');
  assert.equal(input(['KeyW']).longitudinal, 'accelerate');
  const rebound = ['J', 'DownArrow', 'LeftArrow', 'W', 'Return'].map(preferenceKeyCode);
  assert.equal(input(['ArrowUp'], rebound).longitudinal, 'coast');
  assert.equal(input(['KeyJ'], rebound).longitudinal, 'accelerate');
  assert.equal(input(['KeyW'], rebound).longitudinal, 'coast');
  assert.equal(input(['KeyW'], rebound).lateral, 'positive');
  assert.equal(input(['TouchForward'], rebound).longitudinal, 'accelerate');
  assert.equal(input(['TouchTurnLeft'], rebound).roll_axis, -28);
});

test('split map is a distinct uncomposited producer and cannot masquerade as a sprite-complete flight scene', () => {
  const map = packet(1n, 0); new DataView(map.buffer).setUint32(0, 0x314d434d, true);
  assert.throws(() => decodeOriginalFrame(map));
  const frame = decodeOriginalFrame(map, 'map'); assert.equal(frame.kind, 'map');
  assert.throws(() => new OriginalSceneHistory().compose(frame), /Map frames/);
  const falseSprites = map.slice(); new DataView(falseSprites.buffer).setUint32(12, 1, true);
  assert.throws(() => decodeOriginalFrame(falseSprites, 'map'));
});
