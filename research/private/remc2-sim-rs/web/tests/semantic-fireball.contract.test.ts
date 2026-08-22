import assert from "node:assert/strict";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { NeutralSessionClient } from "../headless/neutral-session-client.ts";

type JsonObject = Record<string, any>;

const pluginValue = process.env.MC_GAME_SESSION_PLUGIN;
const workspaceValue = process.env.MC_SEMANTIC_VARIANT_WORKSPACE;
if (pluginValue === undefined || pluginValue.length === 0) {
  throw new Error("MC_GAME_SESSION_PLUGIN must name the exact candidate libsim_capi.so");
}
if (workspaceValue === undefined || workspaceValue.length === 0) {
  throw new Error("MC_SEMANTIC_VARIANT_WORKSPACE must name the published definition workspace");
}
const plugin: string = pluginValue;
const workspace: string = workspaceValue;
const host = fileURLToPath(
  new URL("../../../remc2-unity-native/qualification/neutral_session_host.py", import.meta.url)
);

async function open(workspacePath?: string): Promise<NeutralSessionClient> {
  const client = new NeutralSessionClient("python3", host, plugin, workspacePath);
  await client.negotiate();
  return client;
}

test(
  "TypeScript observes a published fireball cost through the ordinary cast",
  { timeout: 120_000 },
  async () => {
    const baseline = await open();
    const authored = await open(workspace);
    try {
      const command = { cast_left: true };
      await baseline.step("1", "0", command);
      await authored.step("1", "0", command);
      const baselineSnapshot = (await baseline.snapshot()) as JsonObject;
      const authoredSnapshot = (await authored.snapshot()) as JsonObject;
      assert.equal(baselineSnapshot.value.caster_mana, 900);
      assert.equal(authoredSnapshot.value.caster_mana, 800);
      for (const countermetric of [
        "tick",
        "player_pose",
        "player_motion",
        "objective_count",
        "action_sequence",
      ]) {
        assert.deepEqual(baselineSnapshot.value[countermetric], authoredSnapshot.value[countermetric]);
      }
      const baselineEvents = (await baseline.events("1", "1", "0")) as JsonObject;
      const authoredEvents = (await authored.events("1", "1", "0")) as JsonObject;
      const baselineIds = baselineEvents.value.events.map((event: JsonObject) => event.event_id);
      const authoredIds = authoredEvents.value.events.map((event: JsonObject) => event.event_id);
      assert.deepEqual(authoredIds, baselineIds);
      assert.ok(authoredIds.includes("combat.fireball_spawned"));

      const firstValue = structuredClone(authoredSnapshot.value);
      await authored.reset("1", "1");
      await authored.step("2", "0", command);
      assert.deepEqual(((await authored.snapshot()) as JsonObject).value, firstValue);
    } finally {
      await baseline.close();
      await authored.close();
    }
  }
);
