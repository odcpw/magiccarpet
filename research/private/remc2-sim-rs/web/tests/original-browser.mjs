import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { chromium } from '@playwright/test';
import { execFileSync } from 'node:child_process';

// This checks the real installed native backend, never a mocked game.
const output = process.env.ORIGINAL_PROOF;
if (!output?.startsWith('/')) throw new Error('Set ORIGINAL_PROOF to an absolute disk-backed output directory');
await mkdir(output, { recursive: true });
const errors = [], checks = [], routes = [], inputs = [];
let rendering = null;
let mobile = null;
let pendingControls = 0;
const readsDuringControl = [];
const browser = await chromium.launch({ executablePath: process.env.CHROME ?? '/opt/google/chrome/chrome',
  headless: process.env.HEADLESS === '1', args: ['--ozone-platform=x11', '--disable-dev-shm-usage'] });
const page = await browser.newPage({ viewport: { width: 1280, height: 960 }, deviceScaleFactor: 1 });
page.on('pageerror', e => errors.push(String(e)));
page.on('console', m => { if (m.type() === 'error') errors.push(m.text()); });
page.on('response', r => { if (r.status() >= 400) errors.push(`${r.status()} ${r.url()}`); });
page.on('request', r => { if (r.url().endsWith('/control')) console.log('CONTROL', r.postData()); });
page.on('request', r => { if (r.url().includes('/original-api/')) routes.push(new URL(r.url()).pathname); });
page.on('request', r => { if (r.url().endsWith('/original-api/input')) inputs.push(JSON.parse(r.postData()).command); });
page.on('request', r => {
  if (r.url().endsWith('/original-api/control')) pendingControls++;
  else if (pendingControls && /\/original-api\/(world-frame|frame)(\?|$)/.test(r.url())) readsDuringControl.push(r.url());
});
const finished = r => { if (r.url().endsWith('/original-api/control')) pendingControls--; };
page.on('requestfinished', finished); page.on('requestfailed', finished);
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
  if (await page.evaluate(() => !!window.originalWorldMetrics)) {
    await wait(() => window.originalWorldMetrics?.samples > 180);
    rendering = await page.evaluate(() => window.originalWorldMetrics);
    await capture('local-world');
    assert.ok(rendering.triangles > 100 && rendering.sprites > 0);
    checks.push('local terrain and sprite geometry renders');
  }
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
  const beforeSpace = await state(), inputStart = inputs.length;
  await key('Space', 650); await page.waitForTimeout(150);
  const afterSpace = await state();
  assert.equal(afterSpace.hud.local_player_mana, beforeSpace.hud.local_player_mana);
  assert.ok(inputs.slice(inputStart).some(command => command.recover_player === true));
  assert.ok(inputs.slice(inputStart).every(command => !command.cast_selected && !command.cast_left && !command.cast_right));
  checks.push('living Space does not spend mana');
  const beforeCast = await state(); await page.mouse.down({ button: 'right' });
  await page.waitForTimeout(650); await page.mouse.up({ button: 'right' }); await page.waitForTimeout(150);
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
  if (rendering) {
    const context = await browser.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 1, isMobile: true, hasTouch: true });
    await context.addCookies(await page.context().cookies());
    const phone = await context.newPage();
    phone.on('pageerror', e => errors.push(String(e)));
    phone.on('console', m => { if (m.type() === 'error') errors.push(m.text()); });
    phone.on('request', r => { if (r.url().includes('/original-api/')) routes.push(new URL(r.url()).pathname); });
    await phone.goto(process.env.ORIGINAL_URL ?? 'http://127.0.0.1:8878/original.html');
    await phone.waitForFunction(() => !document.querySelector('#play').disabled);
    await phone.locator('#play').tap();
    await phone.waitForFunction(() => window.originalMetrics?.packets > 3);
    const sourceYaw = await phone.evaluate(() => window.originalState.player_pose.yaw);
    const cdp = await context.newCDPSession(phone);
    const touch = async (type, x, y) => cdp.send('Input.dispatchTouchEvent', { type, touchPoints: type === 'touchEnd' ? [] : [{ x, y, id: 1 }] });
    await touch('touchStart', 260, 390);
    for (let i = 1; i <= 6; i++) { await touch('touchMove', 260 + i * 8, 390 - i * 3); await phone.waitForTimeout(40); }
    await touch('touchEnd', 308, 372);
    await phone.waitForFunction(yaw => window.originalState.player_pose.yaw !== yaw, sourceYaw);
    checks.push('touch drag changes authoritative steering');
    const mana = await phone.evaluate(() => window.originalState.hud.local_player_mana);
    const fire = await phone.locator('[data-cast]').boundingBox(); assert.ok(fire);
    await touch('touchStart', fire.x + fire.width / 2, fire.y + fire.height / 2); await phone.waitForTimeout(300); await touch('touchEnd', 0, 0);
    await phone.waitForFunction(mana => window.originalState.hud.local_player_mana < mana, mana);
    checks.push('touch Cast spends native mana');
    await phone.locator('#touch-spells').tap();
    await phone.waitForFunction(() => window.originalState.source_menu_admission?.spells_open);
    await phone.screenshot({ path: `${output}/phone-spells.png` });
    const canvasRect = await phone.locator('#original').boundingBox(); assert.ok(canvasRect);
    const logicalHeight = canvasRect.height * 640 / canvasRect.width;
    await phone.touchscreen.tap(canvasRect.x + 80 * canvasRect.width / 640, canvasRect.y + (logicalHeight - 480 + 424) * canvasRect.width / 640);
    await phone.waitForFunction(() => window.originalState.hud.left.raw_spell_selector === 1);
    await phone.locator('#touch-spells').tap();
    await phone.waitForFunction(() => !window.originalState.source_menu_admission?.spells_open);
    checks.push('touch source grid assigns Possession to left hand');
    const phoneImage = await phone.screenshot({ path: `${output}/phone-playing.png` });
    // Regression for the portrait shader repeating the source HUD halfway
    // down the display. Decode the actual compositor capture, not mock pixels.
    const rgb = execFileSync('ffmpeg', ['-v', 'error', '-i', 'pipe:0', '-frames:v', '1', '-f', 'rawvideo', '-pix_fmt', 'rgb24', 'pipe:1'],
      { input: phoneImage, maxBuffer: 2 * 1024 * 1024 });
    let repeated = 0;
    for (let middle = 273; middle <= 279; middle++) {
      let equal = 0;
      for (let y = 1; y < 28; y++) for (let x = 80; x < 390; x++) {
        const a = (y * 390 + x) * 3, b = ((middle + y) * 390 + x) * 3;
        if (rgb[a] === rgb[b] && rgb[a + 1] === rgb[b + 1] && rgb[a + 2] === rgb[b + 2]) equal++;
      }
      repeated = Math.max(repeated, equal / (27 * 310));
    }
    assert.ok(repeated < .8, `Portrait HUD repeated into the scene (${repeated})`);
    checks.push('portrait screenshot has one top HUD');
    await phone.waitForTimeout(1000);
    mobile = await phone.evaluate(() => window.originalWorldMetrics);
    await phone.locator('#menu').tap();
    await phone.waitForFunction(() => !window.originalMetrics.playing);
    await context.close();
  }
  if (rendering) {
    assert.ok(routes.includes('/original-api/world-frame'));
    assert.ok(!routes.includes('/original-api/frame') && !routes.includes('/original-api/world-map'));
    checks.push('network trace contains no streamed world image requests');
  }
  assert.deepEqual(errors, []);
  assert.deepEqual(readsDuringControl, [], 'scene reads must wait for pending lifecycle controls');
  checks.push('scene polling waits for lifecycle acknowledgement');
  const metrics = await page.evaluate(() => window.originalMetrics);
  await writeFile(`${output}/result.json`, JSON.stringify({ checks, errors, metrics, rendering, mobile, routes: [...new Set(routes)] }, null, 2));
  console.log(JSON.stringify({ checks, errors, metrics, rendering, mobile }));
} catch (error) {
  await capture('failure');
  console.error(error);
  const failed = await state();
  console.error(JSON.stringify({ checks, errors, state: failed && { tick: failed.tick, pose: failed.player_pose, paused: failed.paused },
    ui: await page.evaluate(() => ({ metrics: window.originalMetrics, message: document.querySelector('#message').textContent })) }));
  process.exitCode = 1;
} finally { await browser.close(); }
