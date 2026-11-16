import type { TerrainPlanes } from './original-world-data';

/** ModernShoreMask's source reflection table, with real atlas water pixels. */
export function sourceWaterTypes(atlas: Uint8Array, count: number): Uint8Array {
  if (atlas.length !== 256 * 608 || count !== 152) throw new Error('Unsupported source shoreline atlas');
  const covered = new Uint8Array(256); covered[0] = 1;
  for (let type = 1; type < count; type++) {
    if (!((type >= 35 && type <= 39) || (type >= 44 && type <= 47) || (type >= 80 && type <= 83) ||
      (type >= 102 && type <= 119) || type === 126 || type === 127)) continue;
    for (let z = 0; z < 32 && !covered[type]; z++) for (let x = 0; x < 32; x++)
      if (atlas[(Math.floor(type / 8) * 32 + z) * 256 + (type % 8) * 32 + x]! < 12) { covered[type] = 1; break; }
  }
  return covered;
}
export function sourceWaterAt(planes: TerrainPlanes, at: number, covered: Uint8Array): boolean {
  return !!(planes.angles[at]! & 8) || (planes.heights[at] === 0 && !!covered[planes.types[at]!]!);
}
/** The accepted Unity landform's convex 5x5 reconstruction, presentation only. */
export function modernLandform(planes: TerrainPlanes, visual: Uint8Array, covered: Uint8Array): Float32Array {
  const surface = new Float32Array(65536), at = (x: number, z: number) => (z & 255) * 256 + (x & 255);
  const smooth = (x: number) => { x = Math.max(0, Math.min(1, x)); return x * x * (3 - 2 * x); };
  for (let z = 0; z < 256; z++) for (let x = 0; x < 256; x++) {
    const i = at(x, z), original = visual[i]! * .5; let nearest = 4;
    for (let dz = -3; dz <= 2; dz++) for (let dx = -3; dx <= 2; dx++) if (planes.angles[at(x + dx, z + dz)]! & 8)
      nearest = Math.min(nearest, Math.hypot(Math.max(0, dx, -dx - 1), Math.max(0, dz, -dz - 1)));
    let bank = nearest;
    for (let dz = -1; dz <= 0; dz++) for (let dx = -1; dx <= 0; dx++) {
      const n = at(x + dx, z + dz); if (visual[n] === 0 && covered[planes.types[n]!]!) bank = 0;
    }
    const drop = .45 * (1 - smooth(bank)) * (1 - smooth((original - 1) / 2)), blend = .88 * smooth(nearest / 3);
    let average = 0;
    for (let dz = -2; dz <= 2; dz++) for (let dx = -2; dx <= 2; dx++)
      average += visual[at(x + dx, z + dz)]! * .5 * (dx === 0 ? 6 : Math.abs(dx) === 1 ? 4 : 1) * (dz === 0 ? 6 : Math.abs(dz) === 1 ? 4 : 1);
    surface[i] = original + (average / 256 - original) * blend - drop;
  }
  return surface;
}
export function modernSurfaceHeight(surface: Float32Array, x: number, z: number): number {
  const gx = ((x / 4) % 256 + 256) % 256, gz = ((z / 4) % 256 + 256) % 256;
  const ix = Math.floor(gx), iz = Math.floor(gz), fx = gx - ix, fz = gz - iz;
  const h = (dx: number, dz: number) => surface[((iz + dz) & 255) * 256 + ((ix + dx) & 255)]!;
  const a = h(0, 0), b = h(1, 0), c = h(0, 1), d = h(1, 1);
  if (((ix + iz) & 1) === 0) return fx <= fz ? a + fz * (c - a) + fx * (d - c) : a + fz * (d - b) + fx * (b - a);
  return fx + fz <= 1 ? a + fx * (b - a) + fz * (c - a) : d + (1 - fx) * (c - d) + (1 - fz) * (b - d);
}
