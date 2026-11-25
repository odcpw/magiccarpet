import assert from 'node:assert/strict';
import test from 'node:test';
import { admitsReflection, admitsSprite, assetBytes, decodeTerrain, decodeWorldFrame, orderReflectionRows, reflectionPainterRanks, reflectiveMaterial, sourceProjection, splitWorldPublication, terrainHeight, wrappedDelta, TmapDrawGate, TmapPixelState, type TmapDraw, type WorldSprite } from '../src/original-world-data.ts';

test('bundled world publication splits three byte ranges and rejects a mismatched extent', () => {
  const packet = new ArrayBuffer(22);
  const header = new DataView(packet);
  header.setUint32(0, 0x3142434d, true);
  header.setUint32(4, 1, true);
  header.setUint32(8, 2, true);
  header.setUint32(12, 3, true);
  new Uint8Array(packet, 16).set([1, 2, 3, 4, 5, 6]);
  const { frame, state, observation } = splitWorldPublication(packet);
  assert.deepEqual([...frame], [1]);
  assert.deepEqual([...state], [2, 3]);
  assert.deepEqual([...observation], [4, 5, 6]);
  assert.throws(() => splitWorldPublication(packet.slice(0, 21)));
  header.setUint32(8, 0, true);
  assert.throws(() => splitWorldPublication(packet));
});

test('reflected terrain and actors follow source cell order and admission', () => {
  const sine = new DataView(new ArrayBuffer(2048 * 4)); sine.setInt32(512 * 4, 65536, true);
  const types = new Uint8Array(65536).fill(1);
  const actorCell = 212 * 256 + 118, ownerCell = 213 * 256 + 118;
  const ranks = reflectionPainterRanks({x: 30345, y: 55440, yaw: 0}, sine, types);
  const terrainRank = (cell: number) => ranks.terrain[cell * 4]! + 256 * ranks.terrain[cell * 4 + 1]! + 65536 * ranks.terrain[cell * 4 + 2]!;
  assert.equal(terrainRank(ownerCell) + 1, ranks.actors[actorCell], 'actor follows its source terrain owner');
  assert.ok(terrainRank(actorCell) < ranks.actors[actorCell], 'earlier terrain stays behind this actor');
  assert.equal(ranks.actors[0], 0, 'unvisited cells admit no reflected actors');
  types[ownerCell] = 0;
  const empty = reflectionPainterRanks({x: 30345, y: 55440, yaw: 0}, sine, types);
  assert.equal(empty.terrain[ownerCell * 4], 0);
  assert.equal(empty.terrain[ownerCell * 4 + 1], 0);
  assert.equal(empty.terrain[ownerCell * 4 + 2], 0);
  assert.equal(empty.actors[actorCell], ranks.actors[actorCell], 'zero terrain selector does not suppress the actor chain');
});

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
test('reflection uses source material, eye and current actor admission', () => {
  const row = { ...decodeWorldFrame(packet()).sprites[0]!, entity: 51, x: 1000, y: 900, drawOrder: 0, controlFlags: 0 };
  const eye = { x: 1000, y: 1000, z: 128, yaw: 0 };
  assert.equal(admitsReflection(row, eye, true), true);
  assert.equal(admitsReflection(row, eye, false), false);
  assert.equal(admitsReflection(row, { ...eye, z: 4096 }, true), false);
  assert.equal(admitsReflection({ ...row, controlFlags: 0x20 }, eye, true), false);
  assert.equal(admitsReflection({ ...row, entity: 1000 }, eye, true), false);
  assert.equal(admitsReflection({ ...row, drawOrder: 0xffffffff }, eye, true), false);
  assert.equal(reflectiveMaterial(35), true); assert.equal(reflectiveMaterial(127), true);
  assert.equal(reflectiveMaterial(34), false); assert.equal(reflectiveMaterial(128), false);
  const first = { ...row, entity: 52, x: 1023, y: 990, drawOrder: 0 };
  const second = { ...row, entity: 53, x: 900, y: 800, drawOrder: 1 };
  assert.deepEqual(orderReflectionRows([second, first], eye).map(x => x.entity), [52, 53]);
});
test('source pitch offsets the projection without tilting either viewport', () => {
  const flat = sourceProjection(128, 0, 0, false), pitched = sourceProjection(128, 11, 0, false);
  assert.deepEqual(flat, { x: 328 / 320, y: 328 / 200, offsetX: 0, offsetY: -0 });
  assert.equal(pitched.x, flat.x); assert.equal(pitched.y, flat.y);
  assert.equal(pitched.offsetY, -26 / 200);
  const map = sourceProjection(128, 11, 0, true);
  assert.deepEqual(map, { x: 206 / 128, y: 206 / 200, offsetX: 0, offsetY: -10 / 200 });
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
