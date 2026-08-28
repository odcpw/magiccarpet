import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { createInterface } from "node:readline";

export const PROCESS_CONTRACT = "carpet-flight-process-json/v1";
export const ENGINE_CONTRACT = "carpet-flight-game-session/v3";
export const TRANSPORT_CONTRACT = "carpet-flight-native-json/v3";
export const REQUIRED_CAPABILITIES = [
  "snapshot",
  "step",
  "reset",
  "events",
  "audio",
  "presentation_assets",
  "presentation_entities"
] as const;

type JsonObject = Record<string, unknown>;

interface PendingRequest {
  resolve(value: unknown): void;
  reject(error: Error): void;
}

interface ErrorEnvelope {
  code: string;
  message: string;
  native_status?: number;
  state_changed: false;
}

export class ProcessContractError extends Error {
  readonly code: string;
  readonly nativeStatus?: number;
  readonly stateChanged: false;

  constructor(error: ErrorEnvelope) {
    super(error.message);
    this.name = "ProcessContractError";
    this.code = error.code;
    this.nativeStatus = error.native_status;
    this.stateChanged = error.state_changed;
  }
}

export class NeutralSessionClient {
  readonly #child: ChildProcessWithoutNullStreams;
  readonly #pending = new Map<string, PendingRequest>();
  readonly #stderr: string[] = [];
  #nextId = 1;
  #closed = false;

  constructor(python: string, host: string, plugin: string, workspace?: string) {
    const arguments_ = [host, "--plugin", plugin];
    if (workspace !== undefined) arguments_.push("--workspace", workspace);
    this.#child = spawn(python, arguments_, {
      stdio: ["pipe", "pipe", "pipe"]
    });
    this.#child.stderr.setEncoding("utf8");
    this.#child.stderr.on("data", (chunk: string) => this.#stderr.push(chunk));
    createInterface({ input: this.#child.stdout }).on("line", (line) => {
      try {
        const envelope = JSON.parse(line) as JsonObject;
        const id = envelope.id;
        if (typeof id !== "string") throw new Error("response id is not a string");
        const pending = this.#pending.get(id);
        if (pending === undefined) throw new Error(`unexpected response id ${id}`);
        this.#pending.delete(id);
        if (envelope.ok === true && Object.hasOwn(envelope, "result")) {
          pending.resolve(envelope.result);
        } else if (envelope.ok === false && isErrorEnvelope(envelope.error)) {
          pending.reject(new ProcessContractError(envelope.error));
        } else {
          pending.reject(new Error(`malformed response envelope for ${id}`));
        }
      } catch (error) {
        this.#rejectAll(error instanceof Error ? error : new Error(String(error)));
      }
    });
    this.#child.on("exit", (code, signal) => {
      if (!this.#closed) {
        this.#rejectAll(
          new Error(
            `neutral host exited before close: code=${String(code)} signal=${String(signal)} stderr=${this.#stderr.join("")}`
          )
        );
      }
    });
  }

  #rejectAll(error: Error): void {
    for (const pending of this.#pending.values()) pending.reject(error);
    this.#pending.clear();
  }

  request(operation: string, fields: JsonObject = {}): Promise<unknown> {
    if (this.#closed) return Promise.reject(new Error("neutral host is closed"));
    const id = `ts-${this.#nextId++}`;
    // Reserved envelope fields are owned by the transport even when callers
    // use the generic request method for negative contract tests.
    const request = { ...fields, contract: PROCESS_CONTRACT, id, operation };
    return new Promise((resolve, reject) => {
      this.#pending.set(id, { resolve, reject });
      this.#child.stdin.write(`${JSON.stringify(request)}\n`, (error) => {
        if (error !== null && error !== undefined) {
          this.#pending.delete(id);
          reject(error);
        }
      });
    });
  }

  contract(): Promise<unknown> {
    return this.request("contract");
  }

  negotiate(
    engineContract = ENGINE_CONTRACT,
    transportContract = TRANSPORT_CONTRACT,
    capabilities: readonly string[] = REQUIRED_CAPABILITIES
  ): Promise<unknown> {
    return this.request("negotiate", {
      engine_contract: engineContract,
      transport_contract: transportContract,
      capabilities: [...capabilities]
    });
  }

  snapshot(): Promise<unknown> {
    return this.request("snapshot");
  }

  step(generation: string, expectedTick: string, command: JsonObject): Promise<unknown> {
    return this.request("step", {
      generation,
      expected_tick: expectedTick,
      command
    });
  }

  events(
    generation: string,
    expectedTick: string,
    cursor: string,
    limit = 1024
  ): Promise<unknown> {
    return this.request("events", {
      generation,
      expected_tick: expectedTick,
      cursor,
      limit
    });
  }

  audio(
    generation: string,
    expectedTick: string,
    eventSequence: string
  ): Promise<unknown> {
    return this.request("audio", {
      generation,
      expected_tick: expectedTick,
      event_sequence: eventSequence
    });
  }

  presentationAsset(
    generation: string,
    expectedTick: string,
    semanticId: string
  ): Promise<unknown> {
    return this.request("presentation_asset", {
      generation,
      expected_tick: expectedTick,
      semantic_id: semanticId
    });
  }

  reset(generation: string, expectedTick: string): Promise<unknown> {
    return this.request("reset", { generation, expected_tick: expectedTick });
  }

  async close(): Promise<void> {
    if (this.#closed) return;
    await this.request("close");
    this.#closed = true;
    this.#child.stdin.end();
    if (this.#child.exitCode === null) {
      await new Promise<void>((resolve, reject) => {
        this.#child.once("exit", (code) => {
          if (code === 0) resolve();
          else reject(new Error(`neutral host close failed: code=${String(code)}`));
        });
      });
    }
  }
}

function isErrorEnvelope(value: unknown): value is ErrorEnvelope {
  if (typeof value !== "object" || value === null) return false;
  const record = value as JsonObject;
  return (
    typeof record.code === "string" &&
    typeof record.message === "string" &&
    record.state_changed === false &&
    (record.native_status === undefined || typeof record.native_status === "number")
  );
}

export interface BoundIndexedAsset {
  readonly identity: string;
  readonly contentAddress: string;
  readonly width: number;
  readonly height: number;
  readonly rgba8: Uint8Array;
}

export class IndexedPresentationAssetCache {
  #identity?: string;
  #bound?: BoundIndexedAsset;
  invalidationCount = 0;

  bind(observation: any): BoundIndexedAsset {
    const value = observation?.value;
    const definitionHash = observation?.identity?.definition_hash;
    if (
      typeof definitionHash !== "string" ||
      value?.schema !== "carpet-flight-indexed-sprite/v1" ||
      value?.kind !== "indexed_sprite" ||
      value?.runtime_format !== "indexed8-rgba8/v1" ||
      typeof value.slot_id !== "string" ||
      typeof value.content_address !== "string" ||
      !Number.isInteger(value.width) ||
      !Number.isInteger(value.height) ||
      value.frame_count !== 1 ||
      !Array.isArray(value.palette_rgba8) ||
      !Array.isArray(value.indices)
    ) {
      throw new Error("presentation asset observation is incomplete");
    }
    const identity = `${definitionHash}\0${value.slot_id}\0${value.content_address}`;
    if (this.#identity === identity && this.#bound !== undefined) return this.#bound;
    if (value.palette_rgba8.length === 0 || value.palette_rgba8.length % 4 !== 0) {
      throw new Error("presentation palette is not flattened RGBA8");
    }
    const pixelCount = value.width * value.height;
    if (value.indices.length !== pixelCount) throw new Error("presentation pixel count differs");
    const rgba8 = new Uint8Array(pixelCount * 4);
    const colorCount = value.palette_rgba8.length / 4;
    value.indices.forEach((paletteIndex: unknown, index: number) => {
      if (!Number.isInteger(paletteIndex) || (paletteIndex as number) < 0 ||
          (paletteIndex as number) >= colorCount) {
        throw new Error("presentation index references a missing palette color");
      }
      const source = (paletteIndex as number) * 4;
      rgba8.set(value.palette_rgba8.slice(source, source + 4), index * 4);
    });
    if (this.#bound !== undefined) this.invalidationCount += 1;
    this.#identity = identity;
    this.#bound = {
      identity,
      contentAddress: value.content_address,
      width: value.width,
      height: value.height,
      rgba8
    };
    return this.#bound;
  }
}
