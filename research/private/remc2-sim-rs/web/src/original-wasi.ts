import { ConsoleStdout, Directory, File, OpenDirectory, OpenFile, PreopenDirectory, WASI, wasi as errno } from '@bjorn3/browser_wasi_shim';

type Argument = number | bigint;
type WasmExports = WebAssembly.Exports & { memory: WebAssembly.Memory; mc_wasm_scratch: (bytes: number) => number };
export type Layout = { size: number; fields: Record<string, [number, string | Layout]> };
export type MissionInputs = {
  packages: { package_dir: string; owner_cache_path?: string; presentation_cache_path?: string }[];
  layouts: Record<'Hud' | 'MinimapState' | 'MinimapEntity' | 'MinimapMark' | 'Objective', Layout>;
};

const utf8 = new TextDecoder('utf-8', { fatal: true });
const utf8Encoder = new TextEncoder();

function mount(bytes: ArrayBuffer): PreopenDirectory {
  const root = new PreopenDirectory('/', new Map());
  const data = new DataView(bytes);
  const originals: File[] = [];
  let offset = 0;
  while (offset < bytes.byteLength) {
    const nameLength = data.getUint16(offset, true);
    const size = data.getUint32(offset + 2, true);
    const reference = size === 0xffffffff;
    const length = reference ? 4 : size;
    offset += 6;
    if (!nameLength || offset + nameLength + length > bytes.byteLength) throw new Error('Invalid mission file extent');
    const names = utf8.decode(new Uint8Array(bytes, offset, nameLength)).split('/');
    offset += nameLength;
    if (names.some(name => !name || name === '.' || name === '..')) throw new Error('Invalid mission file path');
    const leaf = names.pop()!;
    let parent = root.dir;
    for (const name of names) {
      const existing = parent.contents.get(name);
      if (existing instanceof File) throw new Error('Invalid mission directory');
      if (existing instanceof Directory) { parent = existing; continue; }
      const directory = new Directory([]);
      parent.contents.set(name, directory);
      parent = directory;
    }
    if (parent.contents.has(leaf)) throw new Error('Duplicate mission file');
    const file = reference ? originals[data.getUint32(offset, true)] : new File([], { readonly: true });
    if (!file) throw new Error('Unknown mission file reference');
    if (!reference) {
      file.data = new Uint8Array(bytes, offset, length);
      originals.push(file);
    }
    parent.contents.set(leaf, file);
    offset += length;
  }
  return root;
}

export class OriginalWasi {
  static async open(module: WebAssembly.Module, files: ArrayBuffer): Promise<OriginalWasi> {
    const log = (line: string) => { if (!line.startsWith('wasi: ')) console.error(line); };
    const wasi = new WASI([], [], [new OpenFile(new File([])), ConsoleStdout.lineBuffered(log),
      ConsoleStdout.lineBuffered(log), mount(files)]);
    // Rust's realpath probes each component with readlink. The input package
    // mounts only regular files and directories, so an existing path is EINVAL.
    wasi.wasiImport.path_readlink = (fd: number, pointer: number, length: number) => {
      const directory = wasi.fds[fd];
      if (!directory) return errno.ERRNO_BADF;
      if (!(directory instanceof OpenDirectory)) return errno.ERRNO_NOTDIR;
      const path = utf8.decode(new Uint8Array(wasi.inst.exports.memory.buffer, pointer, length));
      const { ret } = directory.path_lookup(path, 0);
      return ret || errno.ERRNO_INVAL;
    };
    const instance = await WebAssembly.instantiate(module, { wasi_snapshot_preview1: wasi.wasiImport });
    const exports = instance.exports as WasmExports;
    wasi.initialize({ exports });
    return new OriginalWasi(exports);
  }

  private readonly binary: WasmExports;
  private constructor(binary: WasmExports) { this.binary = binary; }

  scratch(size: number): number {
    const address = this.binary.mc_wasm_scratch(size);
    if (!address) throw new Error('Invalid mission scratch size');
    return address;
  }
  view(): DataView { return new DataView(this.binary.memory.buffer); }
  bytes(address: number, length: number): Uint8Array<ArrayBuffer> {
    return new Uint8Array(this.binary.memory.buffer, address, length);
  }
  private status(name: string, args: Argument[]): number {
    const fn = this.binary[name];
    if (typeof fn !== 'function') throw new Error(`Missing WASM export ${name}`);
    return Number(fn(...args));
  }
  call(name: string, ...args: Argument[]): void {
    const status = this.status(name, args);
    if (!status) return;
    const scratch = this.scratch(16_392);
    this.status('mc_native_last_error_length', [scratch]);
    const length = this.view().getUint32(scratch, true);
    const copied = length <= 16_384 && this.status('mc_native_last_error_copy', [scratch + 8, 16_384, scratch]) === 0;
    const message = copied ? utf8.decode(this.bytes(scratch + 8, length)) : `engine status ${status}`;
    throw new Error(`${name}: ${message}`);
  }
  send(name: string, args: Argument[], value: unknown): void {
    const body = utf8Encoder.encode(JSON.stringify(value));
    const address = this.scratch(body.length);
    this.bytes(address, body.length).set(body);
    this.call(name, ...args, address, body.length);
  }
  read(name: string, args: Argument[], capacity: number): Uint8Array<ArrayBuffer> {
    const scratch = this.scratch(capacity + 8);
    this.call(name, ...args, scratch + 8, capacity, scratch);
    const length = this.view().getUint32(scratch, true);
    if (length > capacity) throw new Error(`Invalid WASM response size from ${name}`);
    return this.bytes(scratch + 8, length).slice();
  }
  json<T>(name: string, args: Argument[], capacity: number): T {
    return JSON.parse(utf8.decode(this.read(name, args, capacity))) as T;
  }
}

export function parseLayout(view: DataView, base: number, layout: Layout): Record<string, any> {
  return Object.fromEntries(Object.entries(layout.fields).map(([key, [offset, kind]]) => {
    const pos = base + offset;
    let value: unknown;
    switch (kind) {
      case 'u8': value = view.getUint8(pos); break;
      case 'i8': value = view.getInt8(pos); break;
      case 'u16': value = view.getUint16(pos, true); break;
      case 'i16': value = view.getInt16(pos, true); break;
      case 'u32': value = view.getUint32(pos, true); break;
      case 'i32': value = view.getInt32(pos, true); break;
      case 'u64': value = view.getBigUint64(pos, true).toString(); break;
      default:
        if (typeof kind !== 'object') throw new Error('Unknown mission layout field');
        value = parseLayout(view, pos, kind);
    }
    return [key, value];
  }));
}
