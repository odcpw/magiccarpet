import assert from 'node:assert/strict';
import test from 'node:test';
import { decodeModernFrame } from '../src/modern-runtime.ts';

function packet(): ArrayBuffer {
  const b = new ArrayBuffer(88 + 52 + 96 + 48 + 96 + 56), v = new DataView(b);
  v.setUint32(0, 0x3144434d, true); v.setUint16(4, 1, true); v.setUint16(6, 88, true);
  v.setBigUint64(8, 9007199254740993n, true); v.setBigUint64(16, 4n, true);
  v.setBigUint64(24, 42n, true); v.setBigUint64(32, 56n, true); v.setBigUint64(40, 2n, true);
  for (const [o, size] of [[48, 52], [56, 96], [64, 48], [72, 96], [80, 56]]) {
    v.setUint32(o!, 1, true); v.setUint32(o! + 4, size!, true);
  }
  let p = 88;
  for (const size of [52, 96, 48, 96, 56]) {
    v.setUint32(p, 2, true); v.setUint32(p + 4, size, true);
    if (size !== 52) { v.setBigUint64(p + 8, 4n, true); v.setBigUint64(p + 16, 42n, true); }
    p += size;
  }
  v.setUint16(96, 3, true); v.setUint16(140 + 32, 8, true);
  v.setUint32(140 + 24, 7, true); v.setInt32(140 + 60, -5, true);
  v.setUint16(236 + 28, 42, true); v.setUint16(284 + 36, 99, true);
  v.setUint16(380 + 28, 77, true); v.setUint8(380 + 52, 2);
  return b;
}
test('MCD1 preserves exact identity and typed observations and rejects corruption', () => {
  const b = packet(), frame = decodeModernFrame(b);
  assert.equal(frame.runtimeGeneration, 9007199254740993n);
  assert.equal(frame.epoch, 2n); assert.equal(frame.entities[0]!.entityIndex, 3);
  assert.equal(frame.characters[0]!.slotGeneration, 7); assert.equal(frame.characters[0]!.life, -5);
  assert.equal(frame.buildings[0]!.sourceSlot, 42); assert.equal(frame.castleChildren[0]!.entityIndex, 99);
  assert.equal(frame.trees[0]!.entityIndex, 77); assert.equal(frame.trees[0]!.classId, 2);
  assert.throws(() => decodeModernFrame(b.slice(0, -1)), /bounds/);
  new DataView(b).setBigUint64(140 + 16, 41n, true);
  assert.throws(() => decodeModernFrame(b), /stale/);
});
