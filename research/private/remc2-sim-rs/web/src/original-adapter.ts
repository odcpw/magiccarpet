/** Browser presentation boundary for the shared native Original renderer. */
export const ORIGINAL_WIDTH = 320;
export const ORIGINAL_HEIGHT = 200;
export const ORIGINAL_PIXELS = ORIGINAL_WIDTH * ORIGINAL_HEIGHT;
export const ORIGINAL_HEADER_BYTES = 56;
export const ORIGINAL_PACKET_BYTES = ORIGINAL_HEADER_BYTES + ORIGINAL_PIXELS + 768;

export interface OriginalFrame {
  readonly kind: 'scene' | 'map';
  readonly flags: number;
  readonly runtimeGeneration: bigint;
  readonly sessionGeneration: bigint;
  readonly tick: bigint;
  readonly sequence: bigint;
  readonly sceneRevision: bigint;
  readonly blurMode: number;
  readonly exitTables: boolean;
  readonly indices: Uint8Array;
  readonly palette: Uint8Array;
}

/** Copy before retaining: no caller-owned memory survives this boundary. */
export function decodeOriginalFrame(input: ArrayBuffer | Uint8Array, kind: 'scene' | 'map' = 'scene'): OriginalFrame {
  const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
  if (bytes.byteLength !== ORIGINAL_PACKET_BYTES) throw new Error('Original frame has an invalid byte length');
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (view.getUint32(0, true) !== (kind === 'scene' ? 0x3146434d : 0x314d434d) || view.getUint16(4, true) !== 1 ||
      view.getUint16(6, true) !== ORIGINAL_HEADER_BYTES) throw new Error('Unsupported Original frame contract');
  if (view.getUint16(8, true) !== ORIGINAL_WIDTH || view.getUint16(10, true) !== ORIGINAL_HEIGHT)
    throw new Error('Original frame dimensions do not match its source renderer');
  const flags = view.getUint32(12, true);
  const blurMode = (flags >>> 1) & 3;
  if (kind === 'map' ? flags !== 0 : (flags & ~15) !== 0 || (blurMode !== 0 && blurMode !== 2) || (flags & 1) === 0)
    throw new Error('Original frame has unsupported flags or incomplete source sprites');
  return {
    kind,
    flags,
    runtimeGeneration: view.getBigUint64(16, true),
    sessionGeneration: view.getBigUint64(24, true),
    tick: view.getBigUint64(32, true),
    sequence: view.getBigUint64(40, true),
    sceneRevision: view.getBigUint64(48, true),
    blurMode,
    exitTables: (flags & 8) !== 0,
    indices: bytes.slice(ORIGINAL_HEADER_BYTES, ORIGINAL_HEADER_BYTES + ORIGINAL_PIXELS),
    palette: bytes.slice(ORIGINAL_HEADER_BYTES + ORIGINAL_PIXELS),
  };
}

export interface IndexedBlendTables { readonly normal: Uint8Array; readonly exit: Uint8Array }

/** Source LABEL_33 history, applied once per native scene revision, before HUD. */
export class OriginalSceneHistory {
  private prior = new Uint8Array(ORIGINAL_PIXELS);
  private next = new Uint8Array(ORIGINAL_PIXELS);
  private identity = '';
  private revision: bigint | null = null;

  reset(): void { this.identity = ''; this.revision = null; }

  compose(frame: OriginalFrame, tables?: IndexedBlendTables): Uint8Array {
    if (frame.kind !== 'scene') throw new Error('Map frames do not enter flight-scene history');
    const identity = `${frame.runtimeGeneration}:${frame.sessionGeneration}`;
    if (identity === this.identity && frame.sceneRevision === this.revision) return this.prior;
    if (identity === this.identity && this.revision !== null && frame.blurMode === 2) {
      const lut = frame.exitTables ? tables?.exit : tables?.normal;
      if (!lut || lut.length !== 65536) throw new Error('Missing Original indexed history table');
      for (let i = 0; i < ORIGINAL_PIXELS; i++) this.next[i] = lut[(frame.indices[i]! << 8) | this.prior[i]!]!;
    } else this.next.set(frame.indices);
    [this.prior, this.next] = [this.next, this.prior];
    this.identity = identity; this.revision = frame.sceneRevision;
    return this.prior;
  }
}

export interface OriginalInput {
  longitudinal: 'coast' | 'accelerate' | 'decelerate';
  lateral: 'neutral' | 'negative' | 'positive';
  roll_axis: number;
  pitch_axis: number;
  cast_selected: boolean;
  cast_left: boolean;
  cast_right: boolean;
  show_objective: boolean;
}

export function originalInput(keys: ReadonlySet<string>, mouseX = 0, mouseY = 0,
  casts: { selected?: boolean; left?: boolean; right?: boolean } = {}): OriginalInput {
  const axis = (value: number) => Math.max(-64, Math.min(64, Math.round(Number.isFinite(value) ? value : 0)));
  return {
    longitudinal: keys.has('KeyW') ? 'accelerate' : keys.has('KeyS') ? 'decelerate' : 'coast',
    lateral: keys.has('KeyA') ? 'negative' : keys.has('KeyD') ? 'positive' : 'neutral',
    roll_axis: axis(mouseX + (keys.has('ArrowRight') ? 28 : 0) - (keys.has('ArrowLeft') ? 28 : 0)),
    pitch_axis: axis(mouseY + (keys.has('ArrowDown') ? 22 : 0) - (keys.has('ArrowUp') ? 22 : 0)),
    cast_selected: casts.selected === true,
    cast_left: casts.left === true,
    cast_right: casts.right === true,
    show_objective: keys.has('KeyO'),
  };
}
