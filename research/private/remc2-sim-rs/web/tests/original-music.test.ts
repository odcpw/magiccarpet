import { test } from 'node:test';
import assert from 'node:assert/strict';
import { OriginalCampaignMusic, type CampaignMusicState } from '../src/original-music.ts';

class FakeGain {
  value = 1;
  gain = { value: 1, cancelScheduledValues() {},
    setValueAtTime: (value: number) => { this.gain.value = value; },
    linearRampToValueAtTime: (value: number) => { this.gain.value = value; } };
  connect() {} disconnect() {}
}
class FakeAudio {
  src = ''; preload = ''; loop = false; paused = true; currentTime = 0;
  plays = 0; loads = 0;
  play(): Promise<void> { this.plays++; this.paused = false; return Promise.resolve(); }
  pause(): void { this.paused = true; }
  removeAttribute(): void { this.src = ''; }
  load(): void { this.loads++; }
}
class FakeContext {
  state = 'running'; currentTime = 0; destination = {};
  gains: FakeGain[] = [];
  createMediaElementSource() { return { connect() {}, disconnect() {} }; }
  createGain() { const gain = new FakeGain(); this.gains.push(gain); return gain; }
}

test('selected source music follows native fade, pause, restart, volume and replacement', async () => {
  const context = new FakeContext(), elements: FakeAudio[] = [];
  const music = new OriginalCampaignMusic('/original-api', () => context as unknown as AudioContext, () => {
    const audio = new FakeAudio(); elements.push(audio); return audio as unknown as HTMLAudioElement;
  });
  const state: CampaignMusicState = { generation: '1', tick: '1', track: 1, playing: true, start_count: 1,
    volume: 127, current_units: 0, target_mode: 1, timer_active: false, fade_callback_count: 0 };
  music.update(state, 4, true, 127, true); await new Promise(resolve => setImmediate(resolve));
  const first = elements[0]!;
  assert.equal(first.src, '/original-api/assets/gameplay-music?revision=4');
  assert.equal(first.loop, true); assert.equal(first.plays, 1);
  assert.equal(context.gains[0]!.gain.value, 0.55);
  music.update({ ...state, current_units: 127, target_mode: 2 }, 4, true, 127, true);
  assert.equal(context.gains[0]!.gain.value, 0.8);
  music.speechMix('duck'); assert.equal(context.gains[1]!.gain.value, 1 / 3);
  music.speechMix('restore'); assert.equal(context.gains[1]!.gain.value, 1);
  music.update(state, 4, false, 127, true); assert.equal(first.paused, true);
  first.currentTime = 17;
  music.update({ ...state, generation: '2', start_count: 2 }, 4, true, 64, true);
  await Promise.resolve();
  assert.equal(first.currentTime, 0); assert.equal(first.plays, 2);
  assert.equal(context.gains[0]!.gain.value, 0.55 * 64 / 127);
  music.update({ ...state, generation: '3', track: 2 }, 5, true, 127, true);
  assert.equal(first.paused, true); assert.equal(first.loads, 1);
  assert.equal(elements[1]!.src, '/original-api/assets/gameplay-music?revision=5');
  music.stop(); assert.equal(elements[1]!.paused, true);
});
