import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { chromium } from '@playwright/test';

// This checks the real installed native backend, never a mocked game.
const output = process.env.ORIGINAL_PROOF;
if (!output?.startsWith('/')) throw new Error('Set ORIGINAL_PROOF to an absolute disk-backed output directory');
await mkdir(output, { recursive: true });
const errors = [], checks = [];
const browser = await chromium.launch({ executablePath: process.env.CHROME ?? '/opt/google/chrome/chrome',
  headless: process.env.HEADLESS === '1', args: ['--ozone-platform=x11', '--disable-dev-shm-usage'] });
const page = await browser.newPage({ viewport: { width: 1280, height: 960 }, deviceScaleFactor: 1 });
page.on('pageerror', e => errors.push(String(e)));
page.on('console', m => { if (m.type() === 'error') errors.push(m.text()); });
page.on('response', r => { if (r.status() >= 400) errors.push(`${r.status()} ${r.url()}`); });
page.on('request', r => { if (r.url().endsWith('/control')) console.log('CONTROL', r.postData()); });
const wait = async (fn, arg) => page.waitForFunction(fn, arg, { timeout: 15000 });
const state = () => page.evaluate(() => window.originalState);
const key = async (code, duration = 150) => {
  await page.keyboard.down(code); await page.waitForTimeout(duration); await page.keyboard.up(code);
};
const capture = name => page.screenshot({ path: `${output}/${name}.png` });
try {
  await page.goto((process.env.ORIGINAL_URL ?? 'http://127.0.0.1:8878/original.html') + '?automation=1');
  await wait(() => !document.querySelector('#play').disabled);
  await page.locator('#play').click();
  await wait(() => window.originalMetrics?.packets > 3);
  assert.ok(await page.locator('#curtain').isHidden(), 'start must not reopen the stale pause overlay');
  // Restart establishes a reproducible beginning without replacing game state locally.
  await page.locator('#menu').click(); await page.locator('#reset').click();
  if (await page.locator('#curtain').isVisible()) await page.locator('#pause').click();
  await wait(() => window.originalMetrics?.playing);
  await capture('opening');
  const initial = await state();
  await page.keyboard.down('w'); await key('ArrowRight', 650); await page.keyboard.up('w');
  await page.waitForTimeout(200);
  const flown = await state();
  assert.notDeepEqual(flown.player_pose, initial.player_pose); checks.push('native flight changes pose');
  await capture('flight');
  await key('o', 350);
  await wait(() => window.originalObservation?.value.objective_textbox_ticks > 0);
  await capture('objective'); checks.push('native objective admission');
  await key('Tab');
  await wait(() => window.originalState?.source_menu_admission?.map_open);
  await capture('map');
  await key('Tab'); await wait(() => !window.originalState?.source_menu_admission?.map_open);
  checks.push('source map opens and closes');
  await page.keyboard.down('Control');
  await wait(() => window.originalState?.source_menu_admission?.spells_open);
  await capture('spells');
  const spells = await page.evaluate(() => window.originalObservation.value.player_spells.map(s => ({ id: s.id, slot: s.menu_slot, unlocked: s.unlocked_tier, available: s.menu_help?.selection_available })));
  assert.ok(spells.some(s => s.id === 'spell.fireball'));
  const rect = await page.locator('#original').boundingBox();
  assert.ok(rect);
  // Assign the original first grid slot to the right caster through real hit testing.
  await page.mouse.click(rect.x + 32 * rect.width / 640, rect.y + 424 * rect.height / 480, { button: 'right' });
  await wait(() => window.originalState?.hud.right.raw_spell_selector === 0);
  checks.push('source grid assigns fireball to right hand');
  await page.keyboard.up('Control');
  await wait(() => !window.originalState?.source_menu_admission?.spells_open);
  checks.push('source spell menu opens and closes');
  const beforeCast = await state(); await key('Space', 650); await page.waitForTimeout(150);
  const afterCast = await state();
  assert.ok(afterCast.hud.local_player_mana < beforeCast.hud.local_player_mana);
  checks.push('casting spends native mana');
  await capture('casting');
  await key('Escape'); await wait(() => !window.originalMetrics.playing);
  await page.waitForTimeout(250); const paused = await state();
  await page.waitForTimeout(250); assert.equal((await state()).tick, paused.tick);
  checks.push('pause stops native ticks');
  await page.locator('#save').click();
  await wait(() => document.querySelector('#message').textContent === 'Game saved.');
  const saved = await state();
  await page.locator('#pause').click(); await key('w', 600); await key('Escape');
  await wait(() => !window.originalMetrics.playing);
  assert.notDeepEqual((await state()).player_pose, saved.player_pose);
  await page.locator('#load').click();
  await wait(pose => JSON.stringify(window.originalState?.player_pose) === JSON.stringify(pose), saved.player_pose);
  assert.equal((await state()).tick, saved.tick); checks.push('persistent save/load restores tick and pose');
  await capture('loaded');
  // Exercise real pointer capture as well as the deterministic keyboard path.
  await page.goto(process.env.ORIGINAL_URL ?? 'http://127.0.0.1:8878/original.html');
  await wait(() => !document.querySelector('#play').disabled);
  await page.locator('#play').click();
  await wait(() => window.originalMetrics?.packets > 3);
  assert.ok(await page.locator('#curtain').isHidden(), 'resume must not reopen the stale pause overlay');
  await page.locator('#original').click({ position: { x: 500, y: 300 } });
  await wait(() => document.pointerLockElement?.id === 'original');
  await page.keyboard.press('Escape');
  await wait(() => !window.originalMetrics.playing);
  checks.push('real pointer capture and Escape pause');
  await page.setViewportSize({ width: 390, height: 844 });
  await capture('phone');
  const fits = await page.locator('#stage').evaluate(el => {
    const r = el.getBoundingClientRect(); return r.width <= innerWidth + 1 && r.height <= innerHeight + 1;
  });
  assert.ok(fits); checks.push('phone viewport fits');
  assert.deepEqual(errors, []);
  const metrics = await page.evaluate(() => window.originalMetrics);
  await writeFile(`${output}/result.json`, JSON.stringify({ checks, errors, metrics }, null, 2));
  console.log(JSON.stringify({ checks, errors, metrics }));
} catch (error) {
  await capture('failure');
  console.error(error);
  const failed = await state();
  console.error(JSON.stringify({ checks, errors, state: failed && { tick: failed.tick, pose: failed.player_pose, paused: failed.paused },
    ui: await page.evaluate(() => ({ metrics: window.originalMetrics, message: document.querySelector('#message').textContent })) }));
  process.exitCode = 1;
} finally { await browser.close(); }
