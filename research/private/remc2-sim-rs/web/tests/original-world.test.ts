import assert from 'node:assert/strict';
import test from 'node:test';
import { admitsSprite, assetBytes, decodeTerrain, decodeWorldFrame, terrainHeight, wrappedDelta, TmapDrawGate, TmapPixelState, type TmapDraw, type WorldSprite } from '../src/original-world-data.ts';

test('native TMAP images replace shared selector pixels and restore the owner base', () => {
  const pixels = new TmapPixelState([{selector: 14, pixels: Uint8Array.of(1, 2, 3, 4)},
    {selector: 19, pixels: Uint8Array.of(5, 6)}]);
  const writes: {selector: number; pixels: number[]}[] = [];
  const upload = (selector: number, bytes: Uint8Array) => writes.push({selector, pixels: [...bytes]});
  pixels.apply([{selector: 14, pixels: 'CQgHBg=='}], upload);
  assert.deepEqual(writes, [{selector: 14, pixels: [9, 8, 7, 6]}]);
  pixels.apply([{selector: 14, pixels: 'CQgHBg=='}], upload);
  assert.equal(writes.length, 1, 'repeated reads do not reupload unchanged selector pixels');
  assert.throws(() => pixels.apply([{selector: 19, pixels: 'CQg='}, {selector: 99, pixels: 'AQ=='}], upload), /Invalid native TMAP/);
  assert.throws(() => pixels.apply([{selector: 14, pixels: 'AQ=='}], upload), /Invalid native TMAP/);
  assert.throws(() => pixels.apply([{selector: 14, pixels: 'CQgHBg=='}, {selector: 14, pixels: 'CQgHBg=='}], upload), /Invalid native TMAP/);
  assert.equal(writes.length, 1, 'malformed batches cannot partially mutate the atlas');
  pixels.apply([], upload);
  assert.deepEqual(writes[1], {selector: 14, pixels: [1, 2, 3, 4]});
  pixels.apply([{selector: 19, pixels: 'CQg='}], upload);
  pixels.apply(undefined, upload);
  assert.deepEqual(writes.slice(2), [{selector: 19, pixels: [9, 8]}, {selector: 19, pixels: [5, 6]}]);
});

test('draw feedback is sorted, deduplicated and bound to the submitted native tick', () => {
  const gate = new TmapDrawGate(), draws: TmapDraw[] = [];
  const frame = {runtimeGeneration: 3n, sessionGeneration: 4n, tick: 5n};
  const emit = (draw: TmapDraw) => draws.push(draw);
  gate.publish(frame, 321, new Set(), emit);
  gate.publish(frame, 321, new Set(), emit);
  gate.publish(frame, 321, new Set([19, 14, 19]), emit);
  gate.publish(frame, 999, new Set([14]), emit);
  gate.publish(frame, 321, new Set([14, 20]), emit);
  gate.publish(frame, 321, new Set([20]), emit);
  gate.publish({...frame, tick: 6n}, 322, new Set([14]), emit);
  gate.publish({...frame, sessionGeneration: 5n}, 323, new Set([19]), emit);
  assert.deepEqual(draws, [{...frame, yaw: 321, selectors: []},
    {...frame, yaw: 321, selectors: [14, 19]},
    {...frame, yaw: 321, selectors: [20]},
    {...frame, tick: 6n, yaw: 322, selectors: [14]},
    {...frame, sessionGeneration: 5n, yaw: 323, selectors: [19]}]);
});

function packet(): Uint8Array {
  const bytes = new Uint8Array(832 + 64), v = new DataView(bytes.buffer);
  v.setUint32(0, 0x3157434d, true); v.setUint16(4, 1, true); v.setUint16(6, 64, true);
  v.setBigUint64(8, 9007199254740993n, true); v.setBigUint64(16, 4n, true);
  v.setBigUint64(24, 42n, true); v.setBigUint64(32, 56n, true); v.setBigUint64(40, 2n, true);
  v.setUint32(48, 1, true); v.setUint32(52, 64, true); v.setUint32(56, 1, true);
  v.setUint32(832, 2, true); v.setUint32(836, 64, true); v.setBigUint64(840, 4n, true); v.setBigUint64(848, 42n, true);
  v.setUint16(864, 153, true); v.setUint16(866, 150, true); v.setUint16(872, 32, true); v.setUint16(874, 65000, true);
  return bytes;
}
test('world packet owns its bytes, preserves u64 identity and rejects stale/native ABI rows', () => {
  const bytes = packet(), frame = decodeWorldFrame(bytes);
  assert.equal(frame.runtimeGeneration, 9007199254740993n); assert.equal(frame.sprites[0]!.selector, 153);
  bytes[64] = 155; assert.equal(frame.palette[0], 0);
  assert.throws(() => decodeWorldFrame(bytes.subarray(1)), /packet|contract/);
  new DataView(bytes.buffer).setBigUint64(848, 41n, true);
  assert.throws(() => decodeWorldFrame(bytes), /Stale/);
  new DataView(bytes.buffer).setBigUint64(848, 42n, true); new DataView(bytes.buffer).setUint32(48, 65535, true);
  assert.throws(() => decodeWorldFrame(bytes), /contract/);
});
test('source sprite admission excludes behind-camera, far and off-chain smoke while retaining wrapped neighbours', () => {
  const row = { ...decodeWorldFrame(packet()).sprites[0]!, x: 1000, y: 900, drawOrder: 0, depthAdjustment: 0 } as WorldSprite;
  const eye = { x: 1000, y: 1000, yaw: 0 };
  assert.equal(admitsSprite(row, eye), true);
  assert.equal(admitsSprite({ ...row, y: 1100 }, eye), false);
  assert.equal(admitsSprite({ ...row, y: 65000 }, { ...eye, y: 100 }), true);
  assert.equal(admitsSprite({ ...row, y: 1000 - 5120 }, eye), false);
  assert.equal(admitsSprite({ ...row, drawOrder: 0xffffffff }, eye), false);
  // The observed oversized smoke regression: these source positions are behind
  // the live player, despite lying inside the old 128 renderer-unit radius.
  assert.equal(admitsSprite({ ...row, x: 20204, y: 56750 }, { x: 22387, y: 49108, yaw: 114 }), false);
  assert.equal(wrappedDelta(2, 1022), 4);
});
test('terrain copies are bounded and source diagonal interpolation wraps at map edges', () => {
  const data = new Uint8Array(327680), planes = decodeTerrain(data);
  data[65536] = 255; assert.equal(planes.heights[0], 0);
  assert.throws(() => decodeTerrain(data.subarray(0, -1)), /Incomplete/);
  planes.heights[0] = 0; planes.heights[1] = 40; planes.heights[256] = 80; planes.heights[257] = 120;
  assert.equal(terrainHeight(planes, 1, 2), 25);
  assert.equal(terrainHeight(planes, 1025, 1026), 25);
  assert.throws(() => assetBytes(data, { offset: 10, length: -1 }), /range/);
  assert.throws(() => assetBytes(data, { offset: data.length, length: 1 }), /range/);
});
