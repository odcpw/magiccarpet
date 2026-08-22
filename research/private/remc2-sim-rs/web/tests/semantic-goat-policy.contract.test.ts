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
  "TypeScript observes the published vigilant Goat policy on the ordinary AI pass",
  { timeout: 120_000 },
  async () => {
    const baseline = await open();
    const authored = await open(workspace);
    try {
      await baseline.step("1", "0", {});
      await authored.step("1", "0", {});
      const baselineSnapshot = (await baseline.snapshot()) as JsonObject;
      const authoredSnapshot = (await authored.snapshot()) as JsonObject;
      for (const countermetric of [
        "tick",
        "player_pose",
        "player_motion",
        "caster_mana",
        "objective_count",
        "action_sequence",
      ]) {
        assert.deepEqual(baselineSnapshot.value[countermetric], authoredSnapshot.value[countermetric]);
      }

      const baselinePage = (await baseline.events("1", "1", "0")) as JsonObject;
      const authoredPage = (await authored.events("1", "1", "0")) as JsonObject;
      const baselineEvents = baselinePage.value.events as JsonObject[];
      const authoredEvents = authoredPage.value.events as JsonObject[];
      const isEngagement = (event: JsonObject): boolean =>
        event.event_id === "ai.engagement_boundary";
      assert.equal(baselineEvents.filter(isEngagement).length, 0);
      const engagements = authoredEvents.filter(isEngagement);
      assert.ok(engagements.length > 0);
      assert.ok(
        engagements.every(
          (event) => typeof event.actor === "string" && typeof event.participant === "string"
        )
      );
      assert.deepEqual(
        authoredEvents.filter((event) => !isEngagement(event)).map((event) => event.event_id),
        baselineEvents.map((event) => event.event_id)
      );

      const firstValue = structuredClone(authoredSnapshot.value);
      const firstEngagements = structuredClone(engagements);
      await authored.reset("1", "1");
      await authored.step("2", "0", {});
      assert.deepEqual(((await authored.snapshot()) as JsonObject).value, firstValue);
      const replayPage = (await authored.events("2", "1", "0")) as JsonObject;
      assert.deepEqual(replayPage.value.events.filter(isEngagement), firstEngagements);
    } finally {
      await baseline.close();
      await authored.close();
    }
  }
);
