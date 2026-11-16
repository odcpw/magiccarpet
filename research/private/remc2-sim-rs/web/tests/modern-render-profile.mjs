import { chromium } from '@playwright/test';
import { writeFile, mkdir } from 'node:fs/promises';
const output = process.env.MODERN_PROOF;
if (!output?.startsWith('/')) throw new Error('Set MODERN_PROOF to an absolute disk-backed directory');
await mkdir(output, { recursive: true });
const browser = await chromium.launch({ executablePath: '/opt/google/chrome/chrome', headless: false,
  args: ['--ozone-platform=x11', '--disable-dev-shm-usage'] });
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 960 } });
  await page.goto(process.env.MODERN_URL ?? 'http://3090.bumblebee-hue.ts.net:8884/?visual=modern&automation=1');
  await page.waitForFunction(() => !document.querySelector('#play').disabled, null, { timeout: 60000 });
  await page.locator('#play').click();
  await page.locator('#menu').click(); await page.locator('#reset').click();
  if (await page.locator('#curtain').isVisible()) await page.locator('#pause').click();
  await page.waitForTimeout(7000);
  const cdp = await page.context().newCDPSession(page);
  const profiling = process.env.MODERN_CPU === '1';
  if (profiling) { await cdp.send('Profiler.enable'); await cdp.send('Profiler.setSamplingInterval', { interval: 1000 }); await cdp.send('Profiler.start'); }
  const sample = await page.evaluate(() => new Promise(resolve => {
    const intervals = [], start = performance.now(), tick = window.modernMetrics.tick;
    let previous = start;
    const measure = now => { intervals.push(now - previous); previous = now;
      if (now - start < 6000) requestAnimationFrame(measure);
      else { const sorted = [...intervals].sort((a, b) => a - b), p = q => sorted[Math.floor((sorted.length - 1) * q)];
        resolve({ frames: intervals.length, fps: 1000 * intervals.length / (now - start), p50_ms: p(.5), p95_ms: p(.95), p99_ms: p(.99), longest_ms: sorted.at(-1), tick_before: tick, tick_after: window.modernMetrics.tick }); }
    }; requestAnimationFrame(measure);
  }));
  const profile = profiling ? (await cdp.send('Profiler.stop')).profile : null;
  const hits = profile?.nodes.map(n => ({ fn: n.callFrame.functionName, url: n.callFrame.url.split('/').at(-1),
    hits: n.hitCount ?? 0, line: n.callFrame.lineNumber })).sort((a, b) => b.hits - a.hits).slice(0, 25) ?? [];
  const live = await page.evaluate(() => {
    const m = window.modernMetrics, particles = {};
    for (const e of window.modernState.entities.filter(e => e.active && e.classId === 10)) {
      const k = e.model + ':' + e.action + ':' + e.phase; particles[k] = (particles[k] ?? 0) + 1;
    }
    return { metrics: { ...m, characters: m.characters.resourceCounts }, particle_models: Object.keys(particles).length };
  });
  await page.screenshot({ path: output + '/steady.png' }); await page.keyboard.press('Escape');
  if (profile) await writeFile(output + '/cpu.json', JSON.stringify(profile));
  await writeFile(output + '/summary.json', JSON.stringify({ warmup_seconds: 7, sample, hits, ...live }, null, 2));
  console.log(JSON.stringify({ sample, hits, ...live }));
} finally { await browser.close(); }
