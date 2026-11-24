import { test } from 'node:test';
import assert from 'node:assert/strict';
import { OriginalSpeech, type ObjectiveSpeechCue } from '../src/original-speech.ts';
import { OriginalGameplayAudio } from '../src/original-audio.ts';

class Node {
  gain = { value: 0, calls: [] as [number, number][],
    setValueAtTime(value: number, time: number) { this.value = value; this.calls.push([value, time]); },
    cancelScheduledValues() {} };
  buffer: unknown; onended: (() => void) | null = null; starts = 0; stops = 0;
  connect() {} disconnect() {} start() { this.starts++; } stop() { this.stops++; }
}
class Context {
  state = 'running'; currentTime = 0; destination = new Node(); sources: Node[] = []; gains: Node[] = [];
  createGain() { const node = new Node(); this.gains.push(node); return node; }
  createBufferSource() { const node = new Node(); this.sources.push(node); return node; }
  async decodeAudioData() { return {} as AudioBuffer; }
  async resume() {}
}
const flush = () => new Promise(resolve => setImmediate(resolve));
const cue: ObjectiveSpeechCue = { generation: '7', sequence: '34', track: 1, segment: 1,
  start_ms: 10999, length_ms: 3999, duck: true };

test('native speech starts once, ducks, and restores when the segment ends', async () => {
  const ctx = new Context(), actions: string[] = [], completed: ObjectiveSpeechCue[] = [], urls: string[] = [], previous = globalThis.fetch;
  globalThis.fetch = async input => { urls.push(String(input)); return new Response(new Uint8Array([0])); };
  try {
    const player = new OriginalSpeech('/original-api', () => ctx as unknown as AudioContext,
      action => actions.push(action), value => completed.push(value));
    player.update([cue], 1, true, 127); await flush();
    player.update([cue], 1, true, 127); await flush();
    assert.equal(ctx.sources.length, 1); assert.equal(ctx.sources[0]!.starts, 1);
    assert.match(urls[0]!, /generation=7&sequence=34&revision=1/);
    assert.equal(actions.at(-1), 'duck');
    ctx.sources[0]!.onended!(); assert.equal(actions.at(-1), 'restore'); assert.deepEqual(completed, [cue]);
    player.update([cue], 1, true, 127); await flush(); assert.equal(ctx.sources.length, 1);
  } finally { globalThis.fetch = previous; }
});

test('speech duck divides source units and restores at 120 Hz', () => {
  const ctx = new Context();
  const audio = new OriginalGameplayAudio('/original-api', () => {}, () => ctx as unknown as AudioContext);
  audio.activate(); audio.speechMix('duck');
  const gain = ctx.gains[0]!.gain;
  assert.equal(gain.value, 42 / 127);
  audio.speechMix('restore');
  assert.equal(gain.calls.at(-1)![0], 1);
  assert.equal(gain.calls.at(-1)![1], 85 / 120);
});

test('muting sound or changing mission stops speech and prevents stale replay', async () => {
  const ctx = new Context(), previous = globalThis.fetch;
  globalThis.fetch = async () => new Response(new Uint8Array([0]));
  try {
    const player = new OriginalSpeech('/original-api', () => ctx as unknown as AudioContext, () => {});
    player.update([cue], 1, true, 127); await flush();
    player.update([cue], 1, false, 0); assert.equal(ctx.sources[0]!.stops, 1);
    player.update([cue], 1, true, 127); await flush(); assert.equal(ctx.sources.length, 1);
    player.update([], 2, true, 127);
    player.update([{...cue, generation: '8'}], 2, true, 127); await flush();
    assert.equal(ctx.sources.length, 2);
  } finally { globalThis.fetch = previous; }
});
