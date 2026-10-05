#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
project_root=$(CDPATH= cd -- "$script_dir/../../.." && pwd -P)

exec sh "$script_dir/run-player.sh" \
  -mc-qualified \
  -screen-fullscreen "${MC_FULLSCREEN:-0}" \
  -screen-width "${MC_WIDTH:-1600}" \
  -screen-height "${MC_HEIGHT:-900}" \
  -mc-package "$project_root/research/private/generated/level1-export-m" \
  -mc-closure "$project_root/research/private/generated/level1-asset-closure.json" \
  -mc-asset-cache "$project_root/research/private/generated/level1-unity-asset-cache-plan29-final" \
  "$@"
