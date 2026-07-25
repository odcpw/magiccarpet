export class AudioDirector {
  #context: AudioContext | null = null;
  #windGain: GainNode | null = null;

  async start(): Promise<void> {
    if (this.#context) {
      await this.#context.resume();
      return;
    }
    const context = new AudioContext();
    const sampleCount = context.sampleRate * 2;
    const noise = context.createBuffer(1, sampleCount, context.sampleRate);
    const samples = noise.getChannelData(0);
    for (let index = 0; index < sampleCount; index += 1) {
      samples[index] = Math.random() * 2 - 1;
    }
    const source = context.createBufferSource();
    source.buffer = noise;
    source.loop = true;
    const filter = context.createBiquadFilter();
    filter.type = "bandpass";
    filter.frequency.value = 520;
    filter.Q.value = 0.55;
    const gain = context.createGain();
    gain.gain.value = 0.025;
    source.connect(filter).connect(gain).connect(context.destination);
    source.start();
    this.#context = context;
    this.#windGain = gain;
  }

  update(speed: number): void {
    if (!this.#context || !this.#windGain) return;
    const volume = 0.018 + Math.min(1, Math.abs(speed) / 80) * 0.12;
    this.#windGain.gain.setTargetAtTime(volume, this.#context.currentTime, 0.12);
  }

  fire(): void {
    if (!this.#context) return;
    const now = this.#context.currentTime;
    const oscillator = this.#context.createOscillator();
    const gain = this.#context.createGain();
    oscillator.type = "sawtooth";
    oscillator.frequency.setValueAtTime(180, now);
    oscillator.frequency.exponentialRampToValueAtTime(62, now + 0.22);
    gain.gain.setValueAtTime(0.09, now);
    gain.gain.exponentialRampToValueAtTime(0.001, now + 0.24);
    oscillator.connect(gain).connect(this.#context.destination);
    oscillator.start(now);
    oscillator.stop(now + 0.25);
  }

  impact(): void {
    if (!this.#context) return;
    const now = this.#context.currentTime;
    const oscillator = this.#context.createOscillator();
    const gain = this.#context.createGain();
    oscillator.type = "triangle";
    oscillator.frequency.setValueAtTime(94, now);
    oscillator.frequency.exponentialRampToValueAtTime(28, now + 0.35);
    gain.gain.setValueAtTime(0.13, now);
    gain.gain.exponentialRampToValueAtTime(0.001, now + 0.38);
    oscillator.connect(gain).connect(this.#context.destination);
    oscillator.start(now);
    oscillator.stop(now + 0.4);
  }
}
