import { test } from 'node:test';
import assert from 'node:assert/strict';
import { decodeOriginalFrame, ORIGINAL_PACKET_BYTES, OriginalSceneHistory, originalInput, markedSpellCycle, markedSpellHand } from '../src/original-adapter.ts';

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

test('split map is a distinct uncomposited producer and cannot masquerade as a sprite-complete flight scene', () => {
  const map = packet(1n, 0); new DataView(map.buffer).setUint32(0, 0x314d434d, true);
  assert.throws(() => decodeOriginalFrame(map));
  const frame = decodeOriginalFrame(map, 'map'); assert.equal(frame.kind, 'map');
  assert.throws(() => new OriginalSceneHistory().compose(frame), /Map frames/);
  const falseSprites = map.slice(); new DataView(falseSprites.buffer).setUint32(12, 1, true);
  assert.throws(() => decodeOriginalFrame(falseSprites, 'map'));
});
