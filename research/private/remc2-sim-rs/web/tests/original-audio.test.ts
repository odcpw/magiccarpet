import { test } from 'node:test';
import assert from 'node:assert/strict';
import { OriginalGameplayAudio, type GameplayVoice } from '../src/original-audio.ts';

class Param {
  value = 0;
  cancelScheduledValues() {}
  setValueAtTime(value: number) { this.value = value; }
  linearRampToValueAtTime(value: number) { this.value = value; }
}
class Node {
  gain = new Param(); playbackRate = new Param(); onended: (() => void) | null = null;
  loop = false; buffer: unknown; starts = 0; stops = 0;
  connect() {} disconnect() {} start() { this.starts++; } stop() { this.stops++; }
}
class Context {
  state = 'running'; currentTime = 0; destination = new Node(); sources: Node[] = [];
  createGain() { return new Node(); }
  createChannelMerger() { return new Node(); }
  createBufferSource() { const n = new Node(); this.sources.push(n); return n; }
  async decodeAudioData() { return {} as AudioBuffer; }
  async resume() {}
}
const flush = () => new Promise(resolve => setImmediate(resolve));
const voice = (changes: Partial<GameplayVoice> = {}): GameplayVoice => ({ generation: '8', sequence: '10',
  voice: 1, sound: 9, rate: 22050, volume_q15: 32767, pan_q16: 32767, sound_volume: 127,
  action: 1, loop: false, target: 127, play_type: 1, ...changes });

test('native one-shot frames play once and completion returns its actual voice', async () => {
  const context = new Context(), completed: GameplayVoice[] = [], previous = globalThis.fetch;
  globalThis.fetch = async () => new Response(new Uint8Array([0]));
  try {
    const player = new OriginalGameplayAudio('/original-api', v => completed.push(v), () => context as unknown as AudioContext);
    player.activate(); const row = voice();
    player.update([row], '8', 2, true, true, 127); await flush();
    player.update([row], '8', 2, true, true, 127); await flush();
    assert.equal(context.sources.length, 1); assert.equal(context.sources[0]!.starts, 1);
    context.sources[0]!.onended!(); assert.deepEqual(completed, [row]);
    player.update([row], '8', 2, true, true, 127); await flush();
    assert.equal(context.sources.length, 1);
  } finally { globalThis.fetch = previous; }
});

test('native loop update/end fade its same voice and pause retires it', async () => {
  const context = new Context(), completed: GameplayVoice[] = [], previous = globalThis.fetch;
  globalThis.fetch = async () => new Response(new Uint8Array([0]));
  try {
    const player = new OriginalGameplayAudio('/original-api', v => completed.push(v), () => context as unknown as AudioContext);
    player.activate();
    const start = voice({sound: 2, loop: true, target: 70});
    player.update([start], '8', 2, true, true, 127); await flush();
    assert.equal(context.sources[0]!.loop, true);
    player.update([start, voice({sequence: '11', sound: 2, action: 7, loop: true, target: 70}),
      voice({sequence: '12', sound: 2, action: 8, loop: true, target: 70})], '8', 2, true, true, 127);
    assert.equal(context.sources.length, 1);
    player.update([], '8', 2, true, false, 127);
    assert.equal(context.sources[0]!.stops, 1); assert.equal(completed.length, 0);
  } finally { globalThis.fetch = previous; }
});
