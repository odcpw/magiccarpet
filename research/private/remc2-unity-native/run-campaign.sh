#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
generated=$(CDPATH= cd -- "$script_dir/../generated" && pwd -P)
run_dir=$(mktemp -d "$generated/campaign-play.XXXXXX")

if [ -n "${MC_CAMPAIGN_REPLAY:-}" ]; then
  set -- -mc-runtime-replay-load "$MC_CAMPAIGN_REPLAY" "$@"
else
  set -- -mc-runtime-menu "$@"
fi

echo "Campaign log: $run_dir/player.log"
exec sh "$script_dir/run-player.sh" \
  -mc-game-runtime \
  -mc-runtime-loading-cache "$generated/level1-unity-asset-cache-plan29-final" \
  -mc-runtime-replay-save "${MC_CAMPAIGN_SAVE:-$generated/campaign-save.json}" \
  -mc-runtime-package "$generated/mc-cf15-level1-package-v3-current" \
  -mc-runtime-owner-cache "$generated/level1-unity-asset-cache-plan29-final" \
  -mc-runtime-presentation-cache "$generated/level1-unity-asset-cache-plan29-final" \
  -mc-runtime-package "$generated/level2-day-block32-campaign-v2" \
  -mc-runtime-owner-cache "$generated/level2-day-spell-owner-cache-v3/components/material" \
  -mc-runtime-presentation-cache "$generated/level2-day-spell-owner-cache-v3" \
  -mc-runtime-package "$generated/level3-day-block32-campaign-recipe-v2" \
  -mc-runtime-owner-cache "$generated/level3-day-spell-owner-cache-v3/components/material" \
  -mc-runtime-presentation-cache "$generated/level3-day-spell-owner-cache-v3" \
  -screen-fullscreen "${MC_FULLSCREEN:-0}" \
  -screen-width "${MC_WIDTH:-1600}" -screen-height "${MC_HEIGHT:-900}" \
  -logFile "$run_dir/player.log" "$@"
