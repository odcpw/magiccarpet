import type { LocalReply } from './original-local-runtime';

/** Worker transport for the same Original renderer and input API. */
export function createLocalGameClient(): typeof fetch {
  const worker = new Worker(new URL('./original-local-worker.ts', import.meta.url), { type: 'module' });
  let nextId = 0;
  let failed: Error | null = null;
  const pending = new Map<number, { reject: (error: Error) => void; resolve: (reply: LocalReply) => void }>();
  worker.onmessage = ({ data }: MessageEvent<LocalReply & { id: number }>) => pending.get(data.id)?.resolve(data);
  worker.onerror = event => {
    failed = new Error(event.message || 'Local game worker failed');
    for (const request of pending.values()) request.reject(failed);
  };
  return async (input, options) => {
    if (failed) throw failed;
    const url = typeof input === 'string' ? input : input instanceof URL ? input.pathname + input.search : input.url;
    if (!url.startsWith('/original-api/')) throw new Error('Invalid local game URL');
    if (options?.body != null && typeof options.body !== 'string') throw new Error('Invalid local game body');
    const id = ++nextId;
    const signal = options?.signal;
    signal?.throwIfAborted();
    const abort = () => pending.get(id)?.reject(new Error('Local game request timed out'));
    try {
      const reply = await new Promise<LocalReply>((resolve, reject) => {
        pending.set(id, { resolve, reject });
        signal?.addEventListener('abort', abort, { once: true });
        worker.postMessage({ id, path: url.slice('/original-api'.length), body: options?.body });
      });
      return new Response(reply.body ?? null, { status: reply.status ?? 200, headers: reply.headers });
    } finally {
      signal?.removeEventListener('abort', abort);
      pending.delete(id);
    }
  };
}
