/** Playback of native-selected objective CD segments, independent of frame rate. */
export interface ObjectiveSpeechCue {
  generation: string; sequence: string; track: number; segment: number;
  start_ms: number; length_ms: number; duck: boolean;
}

type DuckAction = 'duck' | 'restore' | 'reset';
interface SpeechVoice { cue: ObjectiveSpeechCue; source: AudioBufferSourceNode | null; gain: GainNode | null; abort: AbortController }

export class OriginalSpeech {
  private readonly seen = new Set<string>();
  private revision = -1;
  private soundVolume = 127;
  private active: SpeechVoice | null = null;
  private readonly api: string;
  private readonly context: () => AudioContext | null;
  private readonly mix: (action: DuckAction) => void;
  private readonly complete: (cue: ObjectiveSpeechCue) => void;

  constructor(api: string, context: () => AudioContext | null, mix: (action: DuckAction) => void,
    complete: (cue: ObjectiveSpeechCue) => void = () => {}) {
    this.api = api; this.context = context; this.mix = mix; this.complete = complete;
  }

  stop(): void {
    const old = this.active; this.active = null;
    if (old) {
      old.abort.abort();
      if (old.source) { old.source.onended = null; old.source.stop(); old.source.disconnect(); }
      old.gain?.disconnect();
      this.complete(old.cue);
    }
    this.mix('reset');
  }

  update(cues: readonly ObjectiveSpeechCue[], revision: number,
    soundEnabled: boolean, soundVolume: number): void {
    if (this.revision !== revision) { this.stop(); this.revision = revision; this.seen.clear(); }
    if (!Number.isInteger(soundVolume) || soundVolume < 0 || soundVolume > 127)
      throw new Error('Invalid source speech volume');
    this.soundVolume = soundVolume;
    if (!soundEnabled) {
      this.stop();
      for (const cue of cues) {
        const key = `${cue.generation}:${cue.sequence}`;
        if (!this.seen.has(key)) { this.seen.add(key); this.complete(cue); }
      }
      return;
    }
    if (this.active?.gain && this.context())
      this.active.gain.gain.setValueAtTime(soundVolume / 127, this.context()!.currentTime);
    for (const cue of cues) {
      if (!/^[0-9]+$/.test(cue.generation) || !/^[0-9]+$/.test(cue.sequence) ||
          !Number.isInteger(cue.track) || cue.track < 1 || cue.track > 27 ||
          !Number.isInteger(cue.segment) || cue.segment < 0 || cue.segment > 9 ||
          !Number.isInteger(cue.start_ms) || cue.start_ms < 0 ||
          !Number.isInteger(cue.length_ms) || cue.length_ms <= 0 || cue.length_ms > 120000)
        throw new Error('Invalid native objective speech');
      const key = `${cue.generation}:${cue.sequence}`;
      if (this.seen.has(key)) continue;
      this.seen.add(key);
      if (this.seen.size > 64) this.seen.delete(this.seen.values().next().value!);
      this.start(cue, revision);
    }
  }

  private start(cue: ObjectiveSpeechCue, revision: number): void {
    this.stop();
    const owner: SpeechVoice = { cue, source: null, gain: null, abort: new AbortController() };
    this.active = owner;
    if (cue.duck) this.mix('duck');
    void this.play(cue, revision, owner);
  }

  private async play(cue: ObjectiveSpeechCue, revision: number, owner: SpeechVoice): Promise<void> {
    try {
      const context = this.context();
      if (!context || context.state !== 'running') throw new Error('Speech requires a player gesture');
      const query = new URLSearchParams({ generation: cue.generation, sequence: cue.sequence, revision: String(revision) });
      const response = await fetch(`${this.api}/assets/objective-speech?${query}`, {
        signal: AbortSignal.any([owner.abort.signal, AbortSignal.timeout(15000)]) });
      if (!response.ok) throw new Error('Native objective speech segment unavailable');
      const buffer = await context.decodeAudioData(await response.arrayBuffer());
      if (this.active !== owner || this.revision !== revision) return;
      const source = context.createBufferSource(), gain = context.createGain();
      owner.source = source; owner.gain = gain;
      source.buffer = buffer; gain.gain.value = this.soundVolume / 127;
      source.connect(gain); gain.connect(context.destination);
      source.onended = () => {
        source.disconnect(); gain.disconnect();
        if (this.active === owner) { this.active = null; if (cue.duck) this.mix('restore'); this.complete(cue); }
      };
      source.start();
    } catch (error) {
      if (this.active === owner) { this.active = null; if (cue.duck) this.mix('restore'); this.complete(cue); }
      if (!owner.abort.signal.aborted) console.error('Original objective speech:', error);
    }
  }
}
