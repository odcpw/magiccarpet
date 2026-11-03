#!/bin/sh
set -eu
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
# A qualified source snapshot owns its matching plugin, receipt and install lock.
# Keep this optional local pointer separate from the original player's delivery.
modern_project="$script_dir/Builds/Modern"
if [ -e "$modern_project" ] || [ -L "$modern_project" ]; then
    if [ ! -x "$modern_project/run-player.sh" ]; then
        echo "Modern installation is unavailable: $modern_project/run-player.sh" >&2
        exit 1
    fi
    export MC_PLAYER_PROJECT="$modern_project"
fi
exec "$script_dir/run-campaign.sh" -mc-modern "$@"
