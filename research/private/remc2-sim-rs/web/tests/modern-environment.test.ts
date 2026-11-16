import assert from 'node:assert/strict';
import test from 'node:test';
import { sourceWaterTypes, sourceWaterAt, modernLandform, modernSurfaceHeight } from '../src/modern-terrain.ts';
import type { TerrainPlanes } from '../src/original-world-data.ts';

test('calm sea and real dry zero-height ground remain distinct', () => {
  const atlas = new Uint8Array(256 * 608).fill(20);
  const pixel = (type: number) => Math.floor(type / 8) * 32 * 256 + (type % 8) * 32;
  atlas[pixel(35)] = 11; // Reflective transition with a source water pixel.
  atlas[pixel(36)] = 12; // Source palette threshold is strictly below 12.
  atlas[pixel(50)] = 0; // A dark opaque texture is not a water material.
  const covered = sourceWaterTypes(atlas, 152);
  const planes: TerrainPlanes = { types: new Uint8Array([0, 35, 36, 50, 35, 50]), heights: new Uint8Array([0, 0, 0, 0, 20, 0]),
    angles: new Uint8Array([0, 0, 0, 0, 0, 8]), shades: new Uint8Array(6), secondary: new Uint8Array(6) };
  assert.deepEqual(Array.from(planes.types, (_, i) => sourceWaterAt(planes, i, covered)), [true, true, false, false, false, true]);
  assert.throws(() => sourceWaterTypes(atlas.subarray(1), 152));
});

test('visual reconstruction suppresses isolated ridges, preserves flat land and native bytes', () => {
  const heights = new Uint8Array(65536).fill(20), before = heights.slice();
  heights[128 * 256 + 128] = before[128 * 256 + 128] = 100;
  const planes: TerrainPlanes = { types: new Uint8Array(65536).fill(50), heights, angles: new Uint8Array(65536), shades: new Uint8Array(65536), secondary: new Uint8Array(65536) };
  const surface = modernLandform(planes, heights, new Uint8Array(256));
  assert.equal(surface[0], 10);
  assert.ok(Math.abs(surface[128 * 256 + 128]! - 19.75) < .0001);
  assert.deepEqual(heights, before);
  assert.ok(surface.every(v => v >= 10 && v <= 50), 'convex reconstruction must not invent peaks/pits');
  assert.equal(modernSurfaceHeight(surface, 512, 512), surface[128 * 256 + 128]);
  assert.equal(modernSurfaceHeight(surface, -512, 1536), modernSurfaceHeight(surface, 512, 512));
});

test('source sea anchors retain a submerged continuous bed', () => {
  const heights = new Uint8Array(65536), angles = new Uint8Array(65536).fill(8);
  const planes: TerrainPlanes = { heights, angles, types: new Uint8Array(65536), shades: new Uint8Array(65536), secondary: new Uint8Array(65536) };
  const covered = new Uint8Array(256); covered[0] = 1;
  const surface = modernLandform(planes, heights, covered);
  assert.ok(surface.every(v => Math.abs(v + .45) < .0001));
  assert.ok(Math.abs(modernSurfaceHeight(surface, 1023.8, -.2) + .45) < .0001);
});
