/** Present the indexed frames and ordered actions of the native media owner. */
type MediaAction = { op: string; value: number; audio_index: number | null; volume_units?: number | null };
type MediaView = { epoch: number; active: boolean; frame_index: number; width: number; height: number;
  fade_in_level: number; fade_out_level: number; subtitle: string | null; subtitle_viewport_offset: number;
  actions: MediaAction[]; error: string | null; campaign_return?: { phase: string; blocking: boolean } | null };
export type MediaReply = { phase: string; media: MediaView | null };
type Glyph = { index: number; width: number; height: number; mask: string };

export class OriginalMediaPlayer {
  private readonly canvas = document.createElement('canvas');
  private readonly context: CanvasRenderingContext2D;
  private readonly keys = new Set<string>();
  private pointer = false;
  private music: HTMLAudioElement | null = null;
  private readonly samples = new Map<number, HTMLAudioElement>();
  private readonly units = new Map<number, number>();
  private readonly glyphs = new Map<number, { width: number; height: number; mask: Uint8Array }>();
  private closed = false;

  constructor(private readonly stage: HTMLElement, private readonly api: string,
    private readonly step: (elapsed: number, input: boolean, escape: boolean) => Promise<MediaReply>) {
    this.canvas.width = 320; this.canvas.height = 200;
    this.canvas.style.cssText = 'position:absolute;inset:0;width:100%;height:100%;object-fit:contain;background:black;image-rendering:pixelated;z-index:5';
    this.canvas.setAttribute('aria-label', 'Campaign intermission');
    const context = this.canvas.getContext('2d');
    if (!context) throw new Error('Campaign intermission canvas is unavailable');
    this.context = context;
  }

  private keyDown = (event: KeyboardEvent): void => { this.keys.add(event.code); event.stopImmediatePropagation(); };
  private keyUp = (event: KeyboardEvent): void => { this.keys.delete(event.code); event.stopImmediatePropagation(); };
  private pointerDown = (event: PointerEvent): void => { this.pointer = true; event.stopImmediatePropagation(); };
  private pointerUp = (event: PointerEvent): void => { this.pointer = false; event.stopImmediatePropagation(); };

  private async metadata(): Promise<void> {
    const response = await fetch(`${this.api}/media/metadata`, { cache: 'no-store', signal: AbortSignal.timeout(10000) });
    if (!response.ok) throw new Error('Campaign intermission font is unavailable');
    const data = await response.json() as { glyphs: Glyph[] };
    if (!Array.isArray(data.glyphs)) throw new Error('Invalid intermission font');
    for (const glyph of data.glyphs) {
      const mask = Uint8Array.from(atob(glyph.mask), char => char.charCodeAt(0));
      if (!Number.isInteger(glyph.index) || glyph.width < 0 || glyph.height < 0 ||
          mask.length !== glyph.width * glyph.height || this.glyphs.has(glyph.index)) throw new Error('Invalid intermission glyph');
      this.glyphs.set(glyph.index, { width: glyph.width, height: glyph.height, mask });
    }
    if (!this.glyphs.get(65)?.width) throw new Error('Intermission font lacks source glyphs');
  }

  private async audio(action: MediaAction): Promise<void> {
    const op = action.op.toUpperCase();
    if (op === 'X' || op === 'B') { this.music?.pause(); this.music?.remove(); this.music = null; return; }
    if (op === 'E' || (op === 'T' && action.value === 0) || (op === 'S' && action.value === 0)) {
      for (const sample of this.samples.values()) { sample.pause(); sample.remove(); } this.samples.clear(); return;
    }
    if (op === 'T') { this.samples.get(action.value)?.pause(); this.samples.get(action.value)?.remove(); this.samples.delete(action.value); return; }
    if (op === 'G') {
      if (action.volume_units == null) throw new Error('Invalid intermission sample volume');
      this.units.set(action.value, action.volume_units);
      const sample = this.samples.get(action.value); if (sample) sample.volume = Math.max(0, Math.min(1, action.volume_units / 127));
      return;
    }
    const music = ['D', 'L', 'M', 'Z'].includes(op);
    if (!music && !['S', 'R', 'H'].includes(op)) return;
    if (!Number.isSafeInteger(action.audio_index) || action.audio_index! < 0) throw new Error('Missing intermission audio binding');
    const audio = new Audio(`${this.api}/media/audio?index=${action.audio_index}`);
    audio.hidden = true;
    this.stage.append(audio);
    audio.loop = music;
    audio.volume = music ? (op === 'D' ? 100 / 127 : 1) : Math.max(0, Math.min(1, (this.units.get(action.value) ?? 127) / 127));
    if (music) { this.music?.pause(); this.music?.remove(); this.music = audio; }
    else {
      this.samples.get(action.value)?.pause(); this.samples.get(action.value)?.remove(); this.samples.set(action.value, audio);
      audio.addEventListener('ended', () => { if (this.samples.get(action.value) === audio) this.samples.delete(action.value); audio.remove(); }, { once: true });
    }
    await audio.play();
  }

  private subtitle(image: Uint8Array, text: string, white: number): void {
    const cell = this.glyphs.get(65)!;
    const advance = Math.max(1, cell.width), lineHeight = Math.max(1, cell.height);
    const capacity = Math.max(1, Math.floor(315 / advance) - 3);
    const lines: string[] = []; let line = '';
    for (const word of text.split(' ')) {
      if (line && line.length + 1 + word.length > capacity) { lines.push(line); line = ''; }
      line += (line ? ' ' : '') + word;
    }
    if (line) lines.push(line);
    let y = 170;
    for (const row of lines) {
      let x = Math.trunc((315 - advance * row.length) / 2);
      for (const char of row) {
        const glyph = this.glyphs.get(char.charCodeAt(0));
        if (glyph) for (let gy = 0; gy < glyph.height; gy++) for (let gx = 0; gx < glyph.width; gx++)
          if (glyph.mask[gy * glyph.width + gx] && x + gx >= 0 && x + gx < 320 && y + gy < 200)
            image[(y + gy) * 320 + x + gx] = white;
        x += advance;
      }
      y += lineHeight;
    }
  }

  private async draw(view: MediaView): Promise<void> {
    if (!view.active || view.frame_index < 0) return;
    if (view.width !== 320 || view.height !== 200) throw new Error('Invalid intermission frame extent');
    const response = await fetch(`${this.api}/media/frame?epoch=${view.epoch}&frame=${view.frame_index}`,
      { cache: 'no-store', signal: AbortSignal.timeout(10000) });
    if (!response.ok) throw new Error('Campaign intermission frame is unavailable');
    const raw = new Uint8Array(await response.arrayBuffer());
    if (raw.length !== 64768 || view.subtitle_viewport_offset < 0 || view.subtitle_viewport_offset > 64000)
      throw new Error('Invalid intermission frame');
    const image = new Uint8Array(64000), palette = raw.subarray(64000);
    image.set(raw.subarray(view.subtitle_viewport_offset, 64000));
    if (view.subtitle_viewport_offset && view.subtitle) {
      let white = 0, distance = Infinity;
      for (let i = 0; i < 256; i++) {
        const d = [0, 1, 2].reduce((sum, c) => sum + (63 - palette[i * 3 + c]!) ** 2, 0);
        if (d < distance) { white = i; distance = d; }
      }
      this.subtitle(image, view.subtitle, white);
    }
    const frame = this.context.createImageData(320, 200);
    for (let i = 0; i < image.length; i++) {
      const color = image[i]! * 3;
      for (let c = 0; c < 3; c++) {
        const value = Math.trunc(palette[color + c]! * view.fade_in_level / 32);
        frame.data[i * 4 + c] = (value + Math.trunc(-value * view.fade_out_level / 16)) * 4;
      }
      frame.data[i * 4 + 3] = 255;
    }
    this.context.putImageData(frame, 0, 0);
  }

  async run(): Promise<void> {
    this.stage.append(this.canvas);
    window.addEventListener('keydown', this.keyDown, true); window.addEventListener('keyup', this.keyUp, true);
    window.addEventListener('pointerdown', this.pointerDown, true); window.addEventListener('pointerup', this.pointerUp, true);
    let previous = performance.now(), haveFont = false;
    try {
      while (!this.closed) {
        const now = performance.now(), elapsed = Math.min(5_000_000, Math.max(0, Math.round((now - previous) * 1000)));
        previous = now;
        const result = await this.step(elapsed, this.pointer || this.keys.size > 0, this.keys.has('Escape'));
        if (result.phase !== 'media') return;
        if (!haveFont) { await this.metadata(); haveFont = true; }
        const view = result.media;
        if (!view) throw new Error('Native campaign intermission is unavailable');
        if (view.error) throw new Error(view.error);
        if (view.campaign_return?.phase === 'credits')
          throw new Error('Source credits presentation is not yet available in this browser');
        for (const action of view.actions) await this.audio(action);
        await this.draw(view);
        await new Promise<void>(resolve => requestAnimationFrame(() => resolve()));
      }
    } finally { this.dispose(); }
  }

  dispose(): void {
    this.closed = true;
    window.removeEventListener('keydown', this.keyDown, true); window.removeEventListener('keyup', this.keyUp, true);
    window.removeEventListener('pointerdown', this.pointerDown, true); window.removeEventListener('pointerup', this.pointerUp, true);
    this.music?.pause(); this.music?.remove(); for (const sample of this.samples.values()) { sample.pause(); sample.remove(); }
    this.samples.clear(); this.canvas.remove();
  }
}
