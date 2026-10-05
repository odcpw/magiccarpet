#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
generated=$(CDPATH= cd -- "$script_dir/../generated" && pwd -P)
entry_level=
entry_replay=${MC_CAMPAIGN_REPLAY:-}
case "${1:-}" in
  --help)
    echo "Usage: $0 [--level 1|2|3] [Unity player arguments]"
    echo "Default: campaign menu. --level: direct owner-level entry for play."
    exit 0
    ;;
  --level)
    case "${2:-}" in
      1|2|3) entry_level=$2; shift 2 ;;
      *) echo "--level requires 1, 2 or 3" >&2; exit 64 ;;
    esac
    if [ -n "$entry_replay" ]; then
      echo "Use either --level or MC_CAMPAIGN_REPLAY" >&2
      exit 64
    fi
    if [ "$entry_level" != 1 ]; then
      # Rust-generated entry saves retain one neutral opening turn. Level 3
      # also carries the four earlier spells for independent owner-local play.
      if [ "$entry_level" = 3 ]; then
        entry_replay="$generated/opened-l3-block32-castle-boot-c330992-20261005.json"
      else
        entry_replay="$generated/opened-l2-block32-castle-boot-c330992-20261005.json"
      fi
      if [ ! -f "$entry_replay" ]; then
        echo "Missing owner-level entry replay: $entry_replay" >&2
        exit 1
      fi
    fi
    ;;
esac

if [ -n "$entry_replay" ]; then
  set -- -mc-runtime-replay-load "$entry_replay" "$@"
elif [ -z "$entry_level" ]; then
  set -- -mc-runtime-menu "$@"
fi

run_dir=$(mktemp -d "$generated/campaign-play.XXXXXX")
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
