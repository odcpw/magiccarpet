import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  ENGINE_CONTRACT,
  NeutralSessionClient,
  ProcessContractError,
  REQUIRED_CAPABILITIES,
  TRANSPORT_CONTRACT
} from "../headless/neutral-session-client.ts";

type JsonObject = Record<string, any>;

const pluginValue = process.env.MC_GAME_SESSION_PLUGIN;
if (pluginValue === undefined || pluginValue.length === 0) {
  throw new Error("MC_GAME_SESSION_PLUGIN must name the exact candidate libsim_capi.so");
}
const plugin: string = pluginValue;
const host = fileURLToPath(
  new URL("../../../remc2-unity-native/qualification/neutral_session_host.py", import.meta.url)
);
const corpusPath = fileURLToPath(
  new URL("../../../remc2-unity-native/qualification/level1-reference-corpus.json", import.meta.url)
);

function autoplayCommand(index: number): JsonObject {
  let flags = index < 360 ? 1 : 0;
  let roll = 0;
  let pitch = 0;
  if (index < 145) roll = 38;
  else if (index >= 180 && index < 250) [roll, pitch] = [-32, -22];
  else if (index >= 250 && index < 325) pitch = 24;
  if (index >= 325 && index < 365) flags |= 0x08;
  if (index === 160 || index === 300) flags |= 0x10;
  return {
    longitudinal: flags & 1 ? "accelerate" : flags & 2 ? "decelerate" : "coast",
    lateral: flags & 8 ? "positive" : flags & 4 ? "negative" : "neutral",
    roll_axis: roll,
    pitch_axis: pitch,
    cast_left: Boolean(flags & 0x10),
    cast_right: Boolean(flags & 0x20),
    cast_selected: Boolean(flags & 0x40),
    look_yaw: 0,
    look_pitch: 0
  };
}

async function expectContractError(
  promise: Promise<unknown>,
  code: string
): Promise<void> {
  await assert.rejects(promise, (error: unknown) => {
    assert.ok(error instanceof ProcessContractError);
    assert.equal(error.code, code);
    assert.equal(error.stateChanged, false);
    return true;
  });
}

function verifyCheckpoint(
  checkpoint: JsonObject,
  snapshot: JsonObject,
  events: JsonObject,
  verifyLifecycle: boolean
): void {
  const expected = checkpoint.state_identity;
  const identity = snapshot.identity;
  const value = snapshot.value;
  assert.equal(identity.tick, String(expected.tick));
  assert.equal(value.tick, String(expected.tick));
  assert.equal(value.event_head, String(expected.event_head));
  assert.equal(value.state_hash, expected.state_hash_fnv1a64);
  assert.equal(value.conformance_hash, expected.conformance_hash_fnv1a64);
  assert.deepEqual(
    [value.player_pose.x, value.player_pose.y, value.player_pose.z],
    expected.position
  );
  assert.equal(value.player_pose.yaw, expected.yaw);
  assert.equal(value.player_pose.display_pitch, expected.display_pitch);
  assert.equal(value.player_pose.effective_pitch, expected.effective_pitch);
  assert.equal(value.player_pose.roll, expected.roll);
  assert.equal(value.player_motion.target_speed, expected.target_speed);
  assert.equal(value.player_motion.actual_speed, expected.actual_speed);
  assert.equal(value.player_motion.strafe_speed, expected.strafe_speed);
  assert.equal(value.action_sequence, String(expected.fire_sequence));

  const receipt = events.value.compatibility;
  assert.equal(receipt.contract, "mc2-owned-event-audio-identity/v1");
  assert.equal(receipt.event_count, checkpoint.event_identity.count);
  assert.equal(receipt.event_content_sha256, checkpoint.event_identity.content_sha256);
  if (verifyLifecycle) {
    assert.equal(receipt.event_lifecycle_sha256, checkpoint.event_identity.lifecycle_sha256);
  }
  assert.equal(receipt.audio_count, checkpoint.audio_identity.count);
  assert.equal(receipt.audio_content_sha256, checkpoint.audio_identity.content_sha256);
  if (verifyLifecycle) {
    assert.equal(receipt.audio_lifecycle_sha256, checkpoint.audio_identity.lifecycle_sha256);
  }
}

async function openClient(workspace?: string): Promise<NeutralSessionClient> {
  const client = new NeutralSessionClient("python3", host, plugin, workspace);
  const contract = (await client.contract()) as JsonObject;
  assert.equal(contract.engine_contract, ENGINE_CONTRACT);
  assert.equal(contract.transport_contract, TRANSPORT_CONTRACT);
  assert.equal(contract.u64_counter_encoding, "decimal-string");
  assert.equal(contract.u64_hash_encoding, "lowercase-hex-16");
  assert.equal(contract.limits.maximum_event_page_size, 1024);
  assert.deepEqual(contract.capabilities, [...REQUIRED_CAPABILITIES]);
  await client.negotiate();
  return client;
}

async function runRoute(
  client: NeutralSessionClient,
  checkpoints: Map<number, JsonObject>,
  generation: string,
  readCount: number
): Promise<Map<number, JsonObject>> {
  const observations = new Map<number, JsonObject>();
  let cursor = "0";
  let joinedAudio = false;
  for (let index = 0; index < 440; index += 1) {
    for (let poll = 0; poll < readCount; poll += 1) await client.snapshot();
    await client.step(generation, String(index), autoplayCommand(index));
    const tick = index + 1;
    const checkpoint = checkpoints.get(tick);
    if (checkpoint === undefined) continue;
    const snapshot = (await client.snapshot()) as JsonObject;
    const events = (await client.events(generation, String(tick), cursor)) as JsonObject;
    cursor = events.value.next_cursor;
    verifyCheckpoint(checkpoint, snapshot, events, generation === "1");
    if (!joinedAudio) {
      for (const event of events.value.events) {
        const audio = (await client.audio(generation, String(tick), event.sequence)) as JsonObject | null;
        if (audio === null) continue;
        assert.equal(audio.value.event_sequence, event.sequence);
        assert.equal(audio.value.state_hash, event.state_hash);
        assert.deepEqual(audio.value.asset, event.asset);
        assert.equal(typeof audio.value.action, "string");
        joinedAudio = true;
        break;
      }
    }
    observations.set(tick, { snapshot, events });
  }
  assert.equal(observations.size, checkpoints.size);
  assert.equal(joinedAudio, true);
  return observations;
}

function normalized(value: Map<number, JsonObject>): JsonObject {
  const result = Object.fromEntries(structuredClone([...value.entries()]));
  for (const item of Object.values(result) as JsonObject[]) {
    item.snapshot.identity.generation = "0";
    item.events.identity.generation = "0";
    delete item.events.value.compatibility.event_lifecycle_sha256;
    delete item.events.value.compatibility.audio_lifecycle_sha256;
  }
  return result;
}

test(
  "real TypeScript consumer negotiates and preserves the fixed neutral Level 1 route",
  { timeout: 180_000 },
  async () => {
    const corpus = JSON.parse(await readFile(corpusPath, "utf8"));
    const suite = corpus.suites.find(
      (value: JsonObject) => value.id === "unity-synthetic-presentation-route"
    );
    assert.ok(suite);
    const checkpoints = new Map<number, JsonObject>(
      suite.checkpoints.map((value: JsonObject) => [value.unity_checkpoint_tick, value])
    );

    const negative = new NeutralSessionClient("python3", host, plugin);
    try {
      await expectContractError(negative.snapshot(), "NOT_NEGOTIATED");
      await expectContractError(
        negative.negotiate("carpet-flight-game-session/v999"),
        "UNSUPPORTED_CONTRACT"
      );
      await negative.negotiate();
      const initial = (await negative.snapshot()) as JsonObject;
      assert.equal(initial.identity.generation, "1");
      assert.equal(initial.identity.tick, "0");
      const detached = structuredClone(initial);
      detached.value.player_pose.x = 0;
      assert.deepEqual(await negative.snapshot(), initial);
      await expectContractError(
        negative.request("snapshot", { unexpected: true }),
        "INVALID_REQUEST"
      );
      await expectContractError(
        negative.request("step", {
          generation: 1,
          expected_tick: "0",
          command: autoplayCommand(0)
        }),
        "INVALID_REQUEST"
      );
      await expectContractError(
        negative.step("1", "0", { source_opcode: 1 }),
        "INVALID_ARGUMENT"
      );
      assert.deepEqual(await negative.snapshot(), initial);
      await negative.step("1", "0", autoplayCommand(0));
      await expectContractError(negative.step("1", "0", autoplayCommand(0)), "INVALID_ARGUMENT");
      await expectContractError(negative.events("1", "1", "0", 1025), "BOUNDS");
    } finally {
      await negative.close();
    }

    const observations: JsonObject[] = [];
    for (const readCount of [0, 1, 7]) {
      const client = await openClient();
      try {
        const first = await runRoute(client, checkpoints, "1", readCount);
        await client.reset("1", "440");
        const reset = (await client.snapshot()) as JsonObject;
        assert.equal(reset.identity.generation, "2");
        assert.equal(reset.identity.tick, "0");
        await expectContractError(client.step("1", "0", autoplayCommand(0)), "GENERATION_MISMATCH");
        const replay = await runRoute(client, checkpoints, "2", readCount);
        assert.deepEqual(normalized(first), normalized(replay));
        observations.push(normalized(first));
      } finally {
        await client.close();
      }
    }
    assert.deepEqual(observations[0], observations[1]);
    assert.deepEqual(observations[0], observations[2]);
  }
);
