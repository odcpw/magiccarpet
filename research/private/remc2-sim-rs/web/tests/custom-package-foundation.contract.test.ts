import assert from "node:assert/strict";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  IndexedPresentationAssetCache,
  NeutralSessionClient,
} from "../headless/neutral-session-client.ts";

type JsonObject = Record<string, any>;

const plugin = requiredEnvironment("MC_GAME_SESSION_PLUGIN");
const workspaces = {
  custom_baseline: requiredEnvironment("MC_F10_CUSTOM_BASELINE_WORKSPACE"),
  flight: requiredEnvironment("MC_F10_FLIGHT_WORKSPACE"),
  fireball: requiredEnvironment("MC_F10_FIREBALL_WORKSPACE"),
  enemy_policy: requiredEnvironment("MC_F10_AI_WORKSPACE"),
  asset: requiredEnvironment("MC_F10_ASSET_WORKSPACE"),
  combined: requiredEnvironment("MC_F10_COMBINED_WORKSPACE"),
};
const host = fileURLToPath(
  new URL("../../../remc2-unity-native/qualification/neutral_session_host.py", import.meta.url),
);
const assetSlot = "presentation.spell.fireball.projectile_asset";
const engagement = "ai.engagement_boundary";

function requiredEnvironment(name: string): string {
  const value = process.env[name];
  if (value === undefined || value.length === 0) throw new Error(`${name} is required`);
  return value;
}

async function open(workspace?: string): Promise<NeutralSessionClient> {
  const client = new NeutralSessionClient("python3", host, plugin, workspace);
  await client.negotiate();
  return client;
}

async function drive(client: NeutralSessionClient, generation = "1"): Promise<JsonObject> {
  await client.step(generation, "0", { longitudinal: "accelerate" });
  await client.step(generation, "1", { longitudinal: "accelerate", cast_left: true });
  const snapshot = (await client.snapshot()) as JsonObject;
  const events = (await client.events(generation, "2", "0")) as JsonObject;
  return { snapshot, events: events.value.events as JsonObject[] };
}

function eventIds(run: JsonObject): string[] {
  return run.events.map((event: JsonObject) => event.event_id as string);
}

function withoutEngagements(run: JsonObject): string[] {
  return run.events
    .filter((event: JsonObject) => event.event_id !== engagement)
    .map((event: JsonObject) => event.event_id as string);
}

function engagements(run: JsonObject): JsonObject[] {
  return run.events.filter((event: JsonObject) => event.event_id === engagement);
}

function projection(run: JsonObject, asset: JsonObject): JsonObject {
  const value = run.snapshot.value;
  return {
    definition_hash: run.snapshot.identity.definition_hash,
    package_graph_hash: run.snapshot.identity.package_graph_hash,
    tick: value.tick,
    event_head: value.event_head,
    state_hash: value.state_hash,
    conformance_hash: value.conformance_hash,
    player_pose: value.player_pose,
    player_motion: value.player_motion,
    caster_mana: value.caster_mana,
    objective_count: value.objective_count,
    action_sequence: value.action_sequence,
    event_ids: eventIds(run),
    engagement_pairs: engagements(run).map(
      (event) => `${String(event.actor)}>${String(event.participant)}`,
    ),
    asset_content_address: asset.value.content_address,
    asset_indices_base64: Buffer.from(asset.value.indices as number[]).toString("base64"),
  };
}

test(
  "one executable composes all four package-authored effects",
  { timeout: 180_000 },
  async () => {
    const clients: NeutralSessionClient[] = [];
    try {
      const mc2 = await open();
      clients.push(mc2);
      const opened = Object.fromEntries(
        await Promise.all(
          Object.entries(workspaces).map(async ([name, workspace]) => {
            const client = await open(workspace);
            clients.push(client);
            return [name, client] as const;
          }),
        ),
      ) as Record<keyof typeof workspaces, NeutralSessionClient>;

      const mc2Run = await drive(mc2);
      const runs = Object.fromEntries(
        await Promise.all(
          Object.entries(opened).map(async ([name, client]) => [name, await drive(client)] as const),
        ),
      ) as Record<keyof typeof workspaces, JsonObject>;
      const baseline = runs.custom_baseline;
      const baselineValue = baseline.snapshot.value;

      assert.deepEqual(mc2Run.snapshot.value, baselineValue);
      assert.deepEqual(eventIds(mc2Run), eventIds(baseline));

      assert.equal(baselineValue.player_motion.target_speed, 32);
      assert.equal(runs.flight.snapshot.value.player_motion.target_speed, 16);
      assert.notDeepEqual(runs.flight.snapshot.value.player_pose, baselineValue.player_pose);
      assert.equal(runs.flight.snapshot.value.caster_mana, baselineValue.caster_mana);
      assert.deepEqual(eventIds(runs.flight), eventIds(baseline));

      assert.equal(baselineValue.caster_mana, 900);
      assert.equal(runs.fireball.snapshot.value.caster_mana, 800);
      for (const field of ["player_pose", "player_motion", "objective_count", "action_sequence"])
        assert.deepEqual(runs.fireball.snapshot.value[field], baselineValue[field]);
      assert.deepEqual(eventIds(runs.fireball), eventIds(baseline));
      assert.ok(eventIds(runs.fireball).includes("combat.fireball_spawned"));

      assert.equal(engagements(baseline).length, 0);
      assert.ok(engagements(runs.enemy_policy).length > 0);
      assert.ok(
        engagements(runs.enemy_policy).every(
          (event) => typeof event.actor === "string" && typeof event.participant === "string",
        ),
      );
      for (const field of [
        "player_pose", "player_motion", "caster_mana", "objective_count", "action_sequence",
      ]) assert.deepEqual(runs.enemy_policy.snapshot.value[field], baselineValue[field]);
      assert.deepEqual(withoutEngagements(runs.enemy_policy), eventIds(baseline));

      const baselineAsset = (await opened.custom_baseline.presentationAsset("1", "2", assetSlot)) as JsonObject;
      const assetOnly = (await opened.asset.presentationAsset("1", "2", assetSlot)) as JsonObject;
      const combinedAsset = (await opened.combined.presentationAsset("1", "2", assetSlot)) as JsonObject;
      assert.notEqual(assetOnly.value.content_address, baselineAsset.value.content_address);
      assert.deepEqual(runs.asset.snapshot.value, baselineValue);
      assert.deepEqual(eventIds(runs.asset), eventIds(baseline));

      const cache = new IndexedPresentationAssetCache();
      const first = cache.bind(baselineAsset);
      assert.strictEqual(cache.bind(baselineAsset), first);
      const second = cache.bind(assetOnly);
      assert.notStrictEqual(second, first);
      assert.equal(cache.invalidationCount, 1);
      assert.strictEqual(cache.bind(assetOnly), second);

      const combinedValue = runs.combined.snapshot.value;
      assert.equal(combinedValue.player_motion.target_speed,
        runs.flight.snapshot.value.player_motion.target_speed);
      assert.deepEqual(combinedValue.player_pose, runs.flight.snapshot.value.player_pose);
      assert.equal(combinedValue.caster_mana, runs.fireball.snapshot.value.caster_mana);
      assert.deepEqual(
        engagements(runs.combined).map((event) => [event.actor, event.participant]),
        engagements(runs.enemy_policy).map((event) => [event.actor, event.participant]),
      );
      assert.deepEqual(withoutEngagements(runs.combined), eventIds(baseline));
      assert.equal(combinedAsset.value.content_address, assetOnly.value.content_address);

      console.log(`MC_F10_MATRIX ${JSON.stringify(projection(runs.combined, combinedAsset))}`);
    } finally {
      await Promise.all(clients.map(async (client) => client.close()));
    }
  },
);
