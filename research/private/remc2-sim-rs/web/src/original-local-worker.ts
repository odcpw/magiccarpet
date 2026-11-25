/// <reference lib="webworker" />

import { OriginalLocalRuntime, type LocalReply } from './original-local-runtime';
import { OriginalWasi, type MissionInputs } from './original-wasi';

type Request = { id: number; path: string; body?: string };

async function checked(path: string): Promise<Response> {
  const response = await fetch('/original-api/' + path);
  if (!response.ok) throw new Error(`Unable to load local playtest resource ${path} (${response.status})`);
  return response;
}

async function open(): Promise<OriginalLocalRuntime> {
  const [inputs, files, module, atlas] = await Promise.all([
    checked('mission.json').then(response => response.json() as Promise<MissionInputs>),
    checked('mission-files.gz').then(response => {
      if (!response.body) throw new Error('Missing mission package');
      return new Response(response.body.pipeThrough(new DecompressionStream('gzip'))).arrayBuffer();
    }),
    checked('sim_capi.wasm').then(response => WebAssembly.compileStreaming(response)),
    checked('world-assets/manifest').then(response => response.json() as Promise<{
      tmaps: { selector: number; width: number; height: number }[];
    }>),
  ]);
  const core = await OriginalWasi.open(module, files);
  return new OriginalLocalRuntime(core, inputs, new Map(atlas.tmaps.map(row => [row.selector, row.width * row.height])));
}

const scope = self as unknown as DedicatedWorkerGlobalScope;
let runtime: Promise<OriginalLocalRuntime> | undefined;
scope.onmessage = ({ data }: MessageEvent<Request>) => {
  runtime ??= open();
  void runtime.then(engine => engine.request(data.path, data.body)).catch((error: unknown): LocalReply => ({
    status: 500, body: JSON.stringify({ error: error instanceof Error ? error.message : String(error) }),
    headers: { 'Content-Type': 'application/json' },
  })).then(reply => {
    scope.postMessage({ id: data.id, ...reply }, reply.body instanceof ArrayBuffer ? [reply.body] : []);
  });
};
