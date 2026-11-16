#!/usr/bin/env bash
set -euo pipefail
web_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
client_dir="$web_dir/../../generated/original-browser-client"
if [[ ! -d "$web_dir/node_modules" ]]; then
  printf '%s\n' 'Missing browser dependencies. Run pnpm install --frozen-lockfile in the web directory.' >&2
  exit 1
fi
cd "$web_dir"
# This entry uses the native Rust backend and does not build WASM.
pnpm exec vite build --mode original --outDir "$client_dir" --emptyOutDir
exec python3 "$web_dir/original_bridge.py" --renderer geometry --web-root "$client_dir" "$@"
