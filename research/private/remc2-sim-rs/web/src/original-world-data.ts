/** Native geometry publications. No simulation or source selector rules live here. */
export interface WorldSprite {
  entity: number; source: number; selector: number; height: number; mode: number;
  renderParameter: number; baseTmap: number;
  orientation: number; mirrored: boolean; x: number; y: number; z: number; yaw: number;
  pixelMode: number; ownerColor: number; brightness: number; drawOrder: number;
  controlFlags: number; depthAdjustment: number;
}
export interface WorldFrame {
  runtimeGeneration: bigint; sessionGeneration: bigint; tick: bigint; sequence: bigint;
  terrainRevision: bigint; palette: Uint8Array; sprites: WorldSprite[]; staticPalette: boolean;
}
export interface TmapPixels { selector: number; pixels: string }
export interface TmapDraw { runtimeGeneration: bigint; sessionGeneration: bigint; tick: bigint; yaw: number; selectors: number[] }

/** Current native selector pixels, retaining the owner base for reset/unresident rows. */
export class TmapPixelState {
  private readonly base = new Map<number, Uint8Array>();
  private current = new Map<number, Uint8Array>();
  constructor(rows: readonly { selector: number; pixels: Uint8Array }[]) {
    for (const row of rows) {
      if (!Number.isSafeInteger(row.selector) || row.selector < 0 || this.base.has(row.selector)) throw new Error('Invalid source TMAP selector');
      this.base.set(row.selector, row.pixels.slice());
    }
  }
  apply(rows: readonly TmapPixels[] | undefined, upload: (selector: number, pixels: Uint8Array) => void): void {
    const next = new Map<number, Uint8Array>();
    for (const row of rows ?? []) {
      const base = this.base.get(row.selector);
      if (!base || next.has(row.selector) || typeof row.pixels !== 'string' || row.pixels.length !== 4 * Math.ceil(base.length / 3))
        throw new Error('Invalid native TMAP pixels');
      let decoded: string;
      try { decoded = atob(row.pixels); } catch { throw new Error('Invalid native TMAP pixels'); }
      if (decoded.length !== base.length || btoa(decoded) !== row.pixels) throw new Error('Invalid native TMAP pixels');
      next.set(row.selector, Uint8Array.from(decoded, c => c.charCodeAt(0)));
    }
    const touched = new Set([...this.current.keys(), ...next.keys()]);
    for (const selector of touched) {
      const base = this.base.get(selector)!;
      const previous = this.current.get(selector) ?? base, current = next.get(selector) ?? base;
      if (current.some((byte, i) => byte !== previous[i])) upload(selector, current);
    }
    this.current = next;
  }
}

/** Acknowledge each frame, then report newly submitted selectors on redraws. */
export class TmapDrawGate {
  private last = '';
  private readonly seen = new Set<number>();
  publish(frame: Pick<WorldFrame, 'runtimeGeneration' | 'sessionGeneration' | 'tick'>, yaw: number,
    selectors: ReadonlySet<number>, emit: (draw: TmapDraw) => void): void {
    const identity = `${frame.runtimeGeneration}:${frame.sessionGeneration}:${frame.tick}`;
    const first = identity !== this.last;
    if (first) { this.last = identity; this.seen.clear(); }
    const fresh = [...selectors].filter(selector => !this.seen.has(selector)).sort((a, b) => a - b);
    if (!first && !fresh.length) return;
    for (const selector of fresh) this.seen.add(selector);
    emit({ runtimeGeneration: frame.runtimeGeneration, sessionGeneration: frame.sessionGeneration,
      tick: frame.tick, yaw, selectors: fresh });
  }
}
export interface TerrainPlanes {
  types: Uint8Array; heights: Uint8Array; shades: Uint8Array; angles: Uint8Array; secondary: Uint8Array;
}
export interface ByteRange { offset: number; length: number }
export interface WorldAssetManifest {
  schema: string; bytes: number; map_type: number;
  terrain: { width: number; height: number; count: number; indices: ByteRange; map_colors: ByteRange; sine: ByteRange; uv_groups: number[][][] };
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
      renderParameter: v.getUint16(at + 28, true), baseTmap: v.getUint16(at + 30, true),
      selector: v.getUint16(at + 32, true), height: v.getUint16(at + 34, true), mode: v.getUint8(at + 36),
      orientation: v.getUint8(at + 37), mirrored: v.getUint8(at + 38) === 1,
      x: v.getUint16(at + 40, true), y: v.getUint16(at + 42, true), z: v.getInt16(at + 44, true), yaw: v.getInt16(at + 46, true),
      pixelMode: v.getUint8(at + 48), ownerColor: v.getUint8(at + 49), brightness: v.getUint16(at + 50, true),
      drawOrder: v.getUint32(at + 52, true), controlFlags: v.getUint32(at + 56, true),
      depthAdjustment: v.getInt16(at + 60, true) });
  }
  return { runtimeGeneration, sessionGeneration, tick, sequence, terrainRevision,
    palette: bytes.slice(64, 832), sprites, staticPalette: (flags & 1) !== 0 };
}
/** One HTTP response containing three views of the same published world. */
export function splitWorldPublication(input: ArrayBuffer): {
  frame: Uint8Array; state: Uint8Array; observation: Uint8Array;
} {
  const headerBytes = 16;
  if (input.byteLength < headerBytes) throw new Error('Incomplete world publication');
  const header = new DataView(input);
  const frameBytes = header.getUint32(4, true);
  const stateBytes = header.getUint32(8, true);
  const observationBytes = header.getUint32(12, true);
  const stateStart = headerBytes + frameBytes;
  const observationStart = stateStart + stateBytes;
  if (header.getUint32(0, true) !== 0x3142434d || !frameBytes || !stateBytes || !observationBytes ||
      observationStart + observationBytes !== input.byteLength)
    throw new Error('Invalid world publication extent');
  return {
    frame: new Uint8Array(input, headerBytes, frameBytes),
    state: new Uint8Array(input, stateStart, stateBytes),
    observation: new Uint8Array(input, observationStart, observationBytes),
  };
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

/** Source projects pitch as a screen offset, independently of perspective. */
export function sourceProjection(fov: number, pitch: number, roll: number, mapOpen: boolean):
  { x: number; y: number; offsetX: number; offsetY: number } {
  const width = mapOpen ? 128 : 320, height = 200;
  const diagonal = Math.floor(Math.hypot(width, height));
  const scale = Math.max(1, (7 * diagonal * fov) >> 11);
  const offset = (Math.trunc(pitch) * width) >> 8, angle = roll * Math.PI / 1024;
  return { x: 2 * scale / width, y: 2 * scale / height,
    offsetX: 2 * Math.sin(angle) * offset / width, offsetY: -2 * Math.cos(angle) * offset / height };
}

/** Source outdoor lattice painter order, shared by reflected terrain and actors. */
export function reflectionPainterRanks(camera: { x: number; y: number; yaw: number }, sine: DataView,
  types: Uint8Array): { terrain: Uint8Array; actors: Uint32Array } {
  const x = Math.trunc(camera.x) & 65535, y = Math.trunc(camera.y) & 65535;
  const yaw = Math.trunc(camera.yaw) & 2047, quadrant = ((yaw + 256) >> 9) & 3;
  const residual = (((yaw + 256) & 511) - 256) & 2047;
  const sin = sine.getInt32(residual * 4, true), cos = sine.getInt32(((residual + 512) & 2047) * 4, true);
  const xl = x & 255, yl = y & 255, xh = x >> 8, yh = y >> 8;
  const a1 = [-xl - 4864, -yl - 4864, xl - 4864, yl - 4864][quadrant]!;
  const a2 = [yl - 256, -xl, -yl, xl - 256][quadrant]!;
  const eligible = new Uint8Array(840);
  for (let row = 0; row < 21; row++) for (let col = 0; col < 40; col++) {
    const u = a1 + col * 256, v = a2 + row * 256;
    const vx = (Math.imul(u, cos) >> 16) - (Math.imul(v, sin) >> 16);
    const depth = (Math.imul(u, sin) >> 16) + (Math.imul(v, cos) >> 16);
    const distance = (Math.imul(vx, vx) + Math.imul(depth, depth)) >>> 0;
    eligible[row * 40 + col] = depth > -256 && distance < 26214400 ? 1 : 0;
  }
  const terrain = new Uint8Array(65536 * 4), actors = new Uint32Array(65536);
  const split = Math.max(0, Math.min(39, Math.ceil(-a1 / 256) - 1));
  let order = 0;
  for (let row = 20; row >= 1; row--) {
    const columns: number[] = [];
    for (let col = 0; col < split; col++) columns.push(col);
    for (let col = 38; col >= split; col--) columns.push(col);
    for (const col of columns) {
      const index = row * 40 + col, rank = ++order * 1024;
      const tx = [xh - 19 + col, xh + row, xh + 19 - col, xh + 1 - row][quadrant]!;
      const ty = [yh + 1 - row, yh - 19 + col, yh + row, yh + 19 - col][quadrant]!;
      const actorX = tx + [0, 0, -1, -1][quadrant]!, actorY = ty + [-1, 0, 0, -1][quadrant]!;
      if (eligible[index]) actors[((actorY & 255) << 8) | (actorX & 255)] = rank + 2;
      const terrainX = tx + [0, -1, -1, 0][quadrant]!, terrainY = ty + [0, 0, -1, -1][quadrant]!;
      const cell = ((terrainY & 255) << 8) | (terrainX & 255);
      if (eligible[index] && eligible[index + 1] && eligible[index - 39] && eligible[index - 40] && types[cell] !== 0) {
        terrain[cell * 4] = (rank + 1) & 255; terrain[cell * 4 + 1] = ((rank + 1) >> 8) & 255;
        terrain[cell * 4 + 2] = (rank + 1) >> 16; terrain[cell * 4 + 3] = 255;
      }
    }
  }
  return { terrain, actors };
}

/** Source primary-sprite admission, before the GPU's tighter viewport cull. */
export function admitsSprite(row: WorldSprite, camera: { x: number; y: number; yaw: number }): boolean {
  if (row.drawOrder === 0xffffffff) return false;
  const dx = wrappedDelta(row.x, camera.x, 65536), dy = wrappedDelta(row.y, camera.y, 65536);
  const yaw = camera.yaw * Math.PI / 1024;
  return dx * dx + dy * dy < 26214400 && dx * Math.sin(yaw) - dy * Math.cos(yaw) + row.depthAdjustment > 64;
}

/** Source reflected world walk excludes synthetic slots and the no-reflection bit. */
export function admitsReflection(row: WorldSprite, camera: { x: number; y: number; z: number; yaw: number }, enabled: boolean): boolean {
  return enabled && camera.z < 4096 && row.entity > 0 && row.entity < 1000 && (row.controlFlags & 0x20) === 0 &&
    admitsSprite({ ...row, depthAdjustment: 0 }, camera);
}

/** The source's second material table selects indexed mode 26. */
export function reflectiveMaterial(tile: number): boolean {
  return tile === 0 || (tile >= 35 && tile <= 39) || (tile >= 44 && tile <= 47) ||
    (tile >= 80 && tile <= 83) || (tile >= 102 && tile <= 119) || tile === 126 || tile === 127;
}

/** Keep each source cell's live chain intact while ordering cells back to front. */
export function orderReflectionRows(rows: readonly WorldSprite[], camera: { x: number; y: number; yaw: number }): WorldSprite[] {
  const angle = camera.yaw * Math.PI / 1024, sin = Math.sin(angle), cos = Math.cos(angle);
  const tile = (row: WorldSprite) => ((row.y >> 8) << 8) | (row.x >> 8);
  const depth = (row: WorldSprite) => wrappedDelta((row.x & 0xff00) + 128, camera.x, 65536) * sin -
    wrappedDelta((row.y & 0xff00) + 128, camera.y, 65536) * cos;
  return [...rows].sort((a, b) => depth(b) - depth(a) || tile(a) - tile(b) || a.drawOrder - b.drawOrder || a.entity - b.entity);
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
