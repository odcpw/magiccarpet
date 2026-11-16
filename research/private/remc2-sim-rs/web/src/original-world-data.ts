/** Native geometry publications. No simulation or source selector rules live here. */
export interface WorldSprite {
  entity: number; source: number; selector: number; height: number; mode: number;
  orientation: number; mirrored: boolean; x: number; y: number; z: number; yaw: number;
  pixelMode: number; ownerColor: number; brightness: number; drawOrder: number;
  depthAdjustment: number;
}
export interface WorldFrame {
  runtimeGeneration: bigint; sessionGeneration: bigint; tick: bigint; sequence: bigint;
  terrainRevision: bigint; palette: Uint8Array; sprites: WorldSprite[]; staticPalette: boolean;
}
export interface TerrainPlanes {
  types: Uint8Array; heights: Uint8Array; shades: Uint8Array; angles: Uint8Array; secondary: Uint8Array;
}
export interface ByteRange { offset: number; length: number }
export interface WorldAssetManifest {
  schema: string; bytes: number;
  terrain: { width: number; height: number; count: number; indices: ByteRange; map_colors: ByteRange; uv_groups: number[][][] };
  palette: { rgb8: ByteRange; shade_lut: ByteRange; blend_lut: ByteRange; clrn: ByteRange };
  tmaps: { selector: number; width: number; height: number; animated: boolean; frames: ByteRange[] }[];
  unavailable_selectors: number[];
}
export function decodeWorldFrame(input: ArrayBuffer | Uint8Array): WorldFrame {
  const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
  if (bytes.length < 832 || bytes.length > 832 + 4096 * 64) throw new Error('Invalid native world packet length');
  const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const count = v.getUint32(48, true), rowSize = v.getUint32(52, true), flags = v.getUint32(56, true);
  if (v.getUint32(0, true) !== 0x3157434d || v.getUint16(4, true) !== 1 || v.getUint16(6, true) !== 64 ||
    rowSize !== 64 || count > 4096 || bytes.length !== 832 + count * 64 || (flags & ~1) || v.getUint32(60, true))
    throw new Error('Unsupported native world packet contract');
  const runtimeGeneration = v.getBigUint64(8, true), sessionGeneration = v.getBigUint64(16, true);
  const tick = v.getBigUint64(24, true), sequence = v.getBigUint64(32, true), terrainRevision = v.getBigUint64(40, true);
  const sprites: WorldSprite[] = [];
  for (let i = 0; i < count; i++) {
    const at = 832 + i * 64;
    if (v.getUint32(at, true) !== 2 || v.getUint32(at + 4, true) !== 64 ||
      v.getBigUint64(at + 8, true) !== sessionGeneration || v.getBigUint64(at + 16, true) !== tick || v.getUint8(at + 38) > 1)
      throw new Error('Stale or invalid native sprite row');
    sprites.push({ entity: v.getUint16(at + 24, true), source: v.getUint16(at + 26, true),
      selector: v.getUint16(at + 32, true), height: v.getUint16(at + 34, true), mode: v.getUint8(at + 36),
      orientation: v.getUint8(at + 37), mirrored: v.getUint8(at + 38) === 1,
      x: v.getUint16(at + 40, true), y: v.getUint16(at + 42, true), z: v.getInt16(at + 44, true), yaw: v.getInt16(at + 46, true),
      pixelMode: v.getUint8(at + 48), ownerColor: v.getUint8(at + 49), brightness: v.getUint16(at + 50, true),
      drawOrder: v.getUint32(at + 52, true), depthAdjustment: v.getInt16(at + 60, true) });
  }
  return { runtimeGeneration, sessionGeneration, tick, sequence, terrainRevision,
    palette: bytes.slice(64, 832), sprites, staticPalette: (flags & 1) !== 0 };
}
export function decodeTerrain(input: ArrayBuffer | Uint8Array): TerrainPlanes {
  const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
  if (bytes.length !== 5 * 65536) throw new Error('Incomplete native terrain planes');
  return { types: bytes.slice(0, 65536), heights: bytes.slice(65536, 131072), shades: bytes.slice(131072, 196608),
    angles: bytes.slice(196608, 262144), secondary: bytes.slice(262144, 327680) };
}
export function assetBytes(blob: Uint8Array, range: ByteRange, expected?: number): Uint8Array {
  if (!Number.isSafeInteger(range.offset) || !Number.isSafeInteger(range.length) || range.offset < 0 || range.length < 0 ||
    range.offset > blob.length || range.length > blob.length - range.offset || (expected !== undefined && range.length !== expected))
    throw new Error('Source world asset byte range is invalid');
  return blob.subarray(range.offset, range.offset + range.length);
}
/** Shortest displacement in the original toroidal map, in renderer units. */
export function wrappedDelta(value: number, origin: number, width = 1024): number {
  return ((value - origin + width / 2) % width + width) % width - width / 2;
}
export function signedSourceAngle(value: number): number { return ((value + 1024) & 2047) - 1024; }

/** Source primary-sprite admission, before the GPU's tighter viewport cull. */
export function admitsSprite(row: WorldSprite, camera: { x: number; y: number; yaw: number }): boolean {
  if (row.drawOrder === 0xffffffff) return false;
  const dx = wrappedDelta(row.x, camera.x, 65536), dy = wrappedDelta(row.y, camera.y, 65536);
  const yaw = camera.yaw * Math.PI / 1024;
  return dx * dx + dy * dy < 26214400 && dx * Math.sin(yaw) - dy * Math.cos(yaw) + row.depthAdjustment > 64;
}

/** Alternating source-cell diagonals, independently usable for visual grounding. */
export function terrainHeight(planes: TerrainPlanes, x: number, y: number): number {
  const gx = ((x / 4) % 256 + 256) % 256, gy = ((y / 4) % 256 + 256) % 256;
  const ix = Math.floor(gx), iy = Math.floor(gy), fx = gx - ix, fy = gy - iy;
  const h = (dx: number, dy: number) => planes.heights[((iy + dy) & 255) * 256 + ((ix + dx) & 255)]! / 2;
  const a = h(0, 0), b = h(1, 0), c = h(0, 1), d = h(1, 1);
  if (((ix + iy) & 1) === 0) return fx <= fy ? a + fy * (c - a) + fx * (d - c) : a + fy * (d - b) + fx * (b - a);
  return fx + fy <= 1 ? a + fx * (b - a) + fy * (c - a) : d + (1 - fx) * (c - d) + (1 - fy) * (b - d);
}
