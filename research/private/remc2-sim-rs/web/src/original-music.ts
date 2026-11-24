/** The native campaign owns the selected track and adaptive fade state. */
export interface CampaignMusicState {
  generation: string; tick: string; track: number; playing: boolean; start_count: number;
  volume: number; current_units: number; target_mode: number; timer_active: boolean;
  fade_callback_count: number;
}

export class OriginalCampaignMusic {
  private audio: HTMLAudioElement | null = null;
  private source: MediaElementAudioSourceNode | null = null;
  private gain: GainNode | null = null;
  private speechGain: GainNode | null = null;
  private revision = -1;
  private generation = '';
  private startCount = -1;
  private track = 0;
  private playingRequest = false;
  private ducked = false;
  private readonly api: string;
  private readonly context: () => AudioContext | null;
  private readonly makeAudio: () => HTMLAudioElement;

  constructor(api: string, context: () => AudioContext | null,
    makeAudio: () => HTMLAudioElement = () => new Audio()) {
    this.api = api; this.context = context; this.makeAudio = makeAudio;
  }

  pause(): void { this.audio?.pause(); }

  stop(): void {
    if (this.audio) { this.audio.pause(); this.audio.removeAttribute('src'); this.audio.load(); }
    this.source?.disconnect(); this.gain?.disconnect(); this.speechGain?.disconnect();
    this.audio = null; this.source = null; this.gain = null; this.speechGain = null;
    this.playingRequest = false; this.startCount = -1; this.generation = ''; this.track = 0;
  }

  speechMix(action: 'duck' | 'restore' | 'reset'): void {
    this.ducked = action === 'duck';
    const context = this.context(), gain = this.speechGain?.gain;
    if (!context || !gain) return;
    gain.cancelScheduledValues(context.currentTime);
    if (action === 'restore') {
      gain.setValueAtTime(gain.value, context.currentTime);
      gain.linearRampToValueAtTime(1, context.currentTime + 2 / 3);
    } else gain.setValueAtTime(this.ducked ? 1 / 3 : 1, context.currentTime);
  }

  update(state: CampaignMusicState | null, revision: number, enabled: boolean,
    preferenceVolume: number, playing: boolean): void {
    if (!Number.isInteger(preferenceVolume) || preferenceVolume < 0 || preferenceVolume > 127)
      throw new Error('Invalid source music volume');
    if (!state) { this.stop(); return; }
    if (!/^[0-9]+$/.test(state.generation) || !/^[0-9]+$/.test(state.tick) ||
        ![1, 2, 3].includes(state.track) || !Number.isInteger(state.start_count) || state.start_count < 0 ||
        !Number.isInteger(state.volume) || state.volume < 0 || state.volume > 127 ||
        !Number.isInteger(state.current_units) || state.current_units < 0 || state.current_units > 127)
      throw new Error('Invalid native campaign music');
    if (this.revision !== revision || (this.track && this.track !== state.track)) {
      this.stop(); this.revision = revision;
    }
    if (!playing || !enabled || !state.playing) {
      this.audio?.pause();
      return;
    }
    const context = this.context();
    if (!context || context.state !== 'running') return;
    if (!this.audio) {
      const audio = this.makeAudio();
      audio.src = `${this.api}/assets/gameplay-music?revision=${revision}`;
      audio.preload = 'auto'; audio.loop = true;
      const source = context.createMediaElementSource(audio), gain = context.createGain(), speech = context.createGain();
      source.connect(gain); gain.connect(speech); speech.connect(context.destination);
      speech.gain.value = this.ducked ? 1 / 3 : 1;
      this.audio = audio; this.source = source; this.gain = gain; this.speechGain = speech;
      this.track = state.track;
    }
    const audio = this.audio;
    if (this.generation !== state.generation || this.startCount !== state.start_count) {
      audio.pause(); audio.currentTime = 0;
      this.generation = state.generation; this.startCount = state.start_count;
    }
    // OwnerAudioPresentation's whole-track adaptation for the default XMIDI path.
    this.gain!.gain.value = (0.55 + 0.25 * state.current_units / 127) *
      (state.volume / 127) * (preferenceVolume / 127);
    if (audio.paused && !this.playingRequest) {
      this.playingRequest = true;
      void audio.play().catch(error => { if (this.audio === audio) console.error('Original campaign music:', error); })
        .finally(() => { if (this.audio === audio) this.playingRequest = false; });
    }
  }
}
