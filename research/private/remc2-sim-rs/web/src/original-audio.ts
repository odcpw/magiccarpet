/** The native campaign journal owns admission and the shared ten-voice pool. */
export interface GameplayVoice {
  generation: string; sequence: string; voice: number; sound: number; rate: number;
  volume_q15: number; pan_q16: number; sound_volume: number; action: number;
  loop: boolean; target: number; play_type: number;
}

interface VoiceOwner {
  sequence: string; sound: number; loop: boolean; source: AudioBufferSourceNode | null;
  nodes: AudioNode[]; envelope: GainNode | null; rampStart: number; rampFrom: number; rampTarget: number;
}

export class OriginalGameplayAudio {
  private context: AudioContext | null = null;
  private master: GainNode | null = null;
  private readonly decoded = new Map<string, Promise<AudioBuffer>>();
  private readonly voices = new Map<number, VoiceOwner>();
  private generation = '';
  private revision = -1;
  private lastSequence = 0n;

  private readonly api: string;
  private readonly complete: (voice: GameplayVoice) => void;
  private readonly makeContext: () => AudioContext;
  constructor(api: string, complete: (voice: GameplayVoice) => void,
    makeContext: () => AudioContext = () => new AudioContext()) {
    this.api = api; this.complete = complete; this.makeContext = makeContext;
  }

  /** Called synchronously from a player gesture to satisfy browser autoplay. */
  activate(): void {
    if (!this.context) {
      this.context = this.makeContext(); this.master = this.context.createGain();
      this.master.connect(this.context.destination);
    }
    if (this.context.state !== 'running') void this.context.resume();
  }

  private retire(voice: VoiceOwner): void {
    if (voice.source) { voice.source.onended = null; voice.source.stop(); voice.source.disconnect(); }
    for (const node of voice.nodes) node.disconnect();
  }

  stop(): void {
    for (const voice of this.voices.values()) this.retire(voice);
    this.voices.clear();
  }

  private fade(owner: VoiceOwner, target: number): void {
    if (!this.context || !owner.envelope) { owner.rampTarget = target; return; }
    const now = this.context.currentTime;
    const elapsed = Math.max(0, now - owner.rampStart) * 60;
    const current = owner.rampFrom < owner.rampTarget
      ? Math.min(owner.rampTarget, owner.rampFrom + elapsed)
      : Math.max(owner.rampTarget, owner.rampFrom - elapsed);
    owner.rampStart = now; owner.rampFrom = current; owner.rampTarget = target;
    owner.envelope.gain.cancelScheduledValues(now);
    owner.envelope.gain.setValueAtTime(current / 127, now);
    owner.envelope.gain.linearRampToValueAtTime(target / 127, now + Math.abs(target - current) / 60);
  }

  update(rows: readonly GameplayVoice[], generation: string, revision: number,
    enabled: boolean, playing: boolean, soundVolume: number): void {
    if (this.generation !== generation || this.revision !== revision) {
      this.stop(); this.generation = generation; this.revision = revision; this.lastSequence = 0n;
      this.decoded.clear();
    }
    if (this.master && this.context) this.master.gain.setValueAtTime(Math.max(0, Math.min(127, soundVolume)) / 127, this.context.currentTime);
    if (!playing || !enabled) { this.stop(); return; }
    for (const row of [...rows].sort((a, b) => BigInt(a.sequence) < BigInt(b.sequence) ? -1 : 1)) {
      if (row.generation !== generation || !/^[0-9]+$/.test(row.sequence) ||
          !Number.isInteger(row.voice) || row.voice < 0 || row.voice >= 10 ||
          !Number.isInteger(row.sound) || row.sound < 1 || row.sound > 69 ||
          ![1, 2, 7, 8].includes(row.action) || row.volume_q15 < 0 || row.volume_q15 > 32767 ||
          row.pan_q16 < 0 || row.pan_q16 > 65535 || row.target < 0 || row.target > 127)
        throw new Error('Invalid native gameplay voice');
      const sequence = BigInt(row.sequence);
      if (sequence <= this.lastSequence) continue;
      this.lastSequence = sequence;
      const old = this.voices.get(row.voice);
      if (row.action === 7 || row.action === 8) {
        if (old && old.sound === row.sound && old.loop) this.fade(old, row.action === 7 ? row.target : 0);
        else if (old && row.action === 8 && row.play_type === 4 && row.volume_q15 === 512) {
          this.retire(old); this.voices.delete(row.voice);
        }
        continue;
      }
      if (!Number.isInteger(row.rate) || row.rate <= 0) throw new Error('Invalid native gameplay rate');
      if (old) this.retire(old);
      const owner: VoiceOwner = { sequence: row.sequence, sound: row.sound, loop: row.loop, source: null,
        nodes: [], envelope: null, rampStart: 0, rampFrom: 0, rampTarget: row.loop ? row.target : 0 };
      this.voices.set(row.voice, owner);
      void this.play(row, revision, owner);
    }
  }

  private async play(row: GameplayVoice, revision: number, owner: VoiceOwner): Promise<void> {
    try {
      const context = this.context, master = this.master;
      if (!context || !master || context.state !== 'running') throw new Error('Gameplay audio requires a player gesture');
      const key = `${revision}:${row.sound}`;
      let pending = this.decoded.get(key);
      if (!pending) {
        pending = fetch(`${this.api}/assets/gameplay-sound?id=${row.sound}&revision=${revision}`,
          { signal: AbortSignal.timeout(15000) })
          .then(async response => {
            if (!response.ok) throw new Error('Selected gameplay sound is unavailable');
            return context.decodeAudioData(await response.arrayBuffer());
          }).catch(error => {
            if (this.decoded.get(key) === pending) this.decoded.delete(key);
            throw error;
          });
        this.decoded.set(key, pending);
      }
      const buffer = await pending;
      if (this.voices.get(row.voice) !== owner || revision !== this.revision) return;
      const source = context.createBufferSource(); owner.source = source;
      source.buffer = buffer; source.loop = row.loop; source.playbackRate.value = row.rate / 22050;
      const pan = row.loop || row.play_type === 4 ? 64 : Math.floor(127 * row.pan_q16 / 65535);
      const volume = row.loop ? 1 : (row.volume_q15 >> 8) / 127;
      const left = context.createGain(), right = context.createGain(), stereo = context.createChannelMerger(2), envelope = context.createGain();
      owner.nodes = [left, right, stereo, envelope]; owner.envelope = envelope;
      left.gain.value = volume * (127 - pan) / 127; right.gain.value = volume * pan / 127;
      source.connect(left); source.connect(right); left.connect(stereo, 0, 0); right.connect(stereo, 0, 1);
      stereo.connect(envelope); envelope.connect(master);
      if (row.loop) { envelope.gain.value = 0; const target = owner.rampTarget; owner.rampTarget = 0; owner.rampStart = context.currentTime; this.fade(owner, target); }
      source.onended = () => {
        source.disconnect(); for (const node of owner.nodes) node.disconnect();
        if (this.voices.get(row.voice) === owner) { this.voices.delete(row.voice); if (!row.loop) this.complete(row); }
      };
      source.start();
    } catch (error) {
      if (this.voices.get(row.voice) === owner) { this.voices.delete(row.voice); this.complete(row); }
      console.error('Original gameplay audio:', error);
    }
  }
}
