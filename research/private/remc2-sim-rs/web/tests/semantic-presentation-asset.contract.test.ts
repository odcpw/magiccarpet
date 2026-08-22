import assert from "node:assert/strict";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  IndexedPresentationAssetCache,
  NeutralSessionClient
} from "../headless/neutral-session-client.ts";

const plugin = process.env.MC_GAME_SESSION_PLUGIN;
const baselineWorkspace = process.env.MC_SEMANTIC_BASELINE_WORKSPACE;
const variantWorkspace = process.env.MC_SEMANTIC_VARIANT_WORKSPACE;
if (!plugin || !baselineWorkspace || !variantWorkspace) {
  throw new Error("F9 requires plugin plus baseline and variant workspaces");
}
const host = fileURLToPath(
  new URL("../../../remc2-unity-native/qualification/neutral_session_host.py", import.meta.url)
);
const slot = "presentation.spell.fireball.projectile_asset";

async function opened(workspace: string): Promise<NeutralSessionClient> {
  const client = new NeutralSessionClient("python3", host, plugin!, workspace);
  await client.negotiate();
  return client;
}

test("typed asset reaches the TypeScript payload cache with causal invalidation", async () => {
  const baseline = await opened(baselineWorkspace!);
  const variant = await opened(variantWorkspace!);
  try {
    const baselineSnapshot: any = await baseline.snapshot();
    const variantSnapshot: any = await variant.snapshot();
    assert.equal(variantSnapshot.value.state_hash, baselineSnapshot.value.state_hash);
    assert.equal(variantSnapshot.value.conformance_hash, baselineSnapshot.value.conformance_hash);
    assert.equal(variantSnapshot.value.event_head, baselineSnapshot.value.event_head);

    const baselineAsset: any = await baseline.presentationAsset("1", "0", slot);
    const variantAsset: any = await variant.presentationAsset("1", "0", slot);
    assert.equal(baselineAsset.value.slot_id, slot);
    assert.equal(variantAsset.value.slot_id, slot);
    assert.notEqual(
      variantAsset.value.content_address,
      baselineAsset.value.content_address
    );

    const cache = new IndexedPresentationAssetCache();
    const first = cache.bind(baselineAsset);
    assert.equal(first.width, 3);
    assert.equal(first.height, 3);
    assert.strictEqual(cache.bind(baselineAsset), first);
    assert.equal(cache.invalidationCount, 0);
    const second = cache.bind(variantAsset);
    assert.notStrictEqual(second, first);
    assert.equal(cache.invalidationCount, 1);
    assert.strictEqual(cache.bind(variantAsset), second);
    assert.equal(cache.invalidationCount, 1);
    assert.notDeepEqual(second.rgba8, first.rgba8);
  } finally {
    await baseline.close();
    await variant.close();
  }
});
