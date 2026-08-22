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
  "TypeScript observes a published flight definition through the ordinary engine step",
  { timeout: 120_000 },
  async () => {
    const baseline = await open();
    const authored = await open(workspace);
    try {
      const command = { longitudinal: "accelerate" };
      for (const tick of ["0", "1"]) {
        await baseline.step("1", tick, command);
        await authored.step("1", tick, command);
      }
      const baselineSnapshot = (await baseline.snapshot()) as JsonObject;
      const authoredSnapshot = (await authored.snapshot()) as JsonObject;
      assert.equal(baselineSnapshot.value.player_motion.target_speed, 32);
      assert.equal(authoredSnapshot.value.player_motion.target_speed, 16);
      assert.notEqual(
        baselineSnapshot.value.player_motion.actual_speed,
        authoredSnapshot.value.player_motion.actual_speed
      );
      assert.notDeepEqual(baselineSnapshot.value.player_pose, authoredSnapshot.value.player_pose);
      for (const countermetric of ["tick", "caster_mana", "objective_count", "action_sequence"]) {
        assert.equal(baselineSnapshot.value[countermetric], authoredSnapshot.value[countermetric]);
      }
      const baselineEvents = (await baseline.events("1", "2", "0")) as JsonObject;
      const authoredEvents = (await authored.events("1", "2", "0")) as JsonObject;
      assert.deepEqual(
        baselineEvents.value.events.map((event: JsonObject) => event.event_id),
        authoredEvents.value.events.map((event: JsonObject) => event.event_id)
      );

      const firstValue = structuredClone(authoredSnapshot.value);
      await authored.reset("1", "2");
      for (const tick of ["0", "1"]) await authored.step("2", tick, command);
      assert.deepEqual(((await authored.snapshot()) as JsonObject).value, firstValue);
    } finally {
      await baseline.close();
      await authored.close();
    }
  }
);
