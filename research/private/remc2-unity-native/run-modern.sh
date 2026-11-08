#!/bin/sh
set -eu
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
# A qualified source snapshot owns its matching plugin, receipt and install lock.
# Keep this optional local pointer separate from the original player's delivery.
modern_project="$script_dir/Builds/Modern"
sandbox=0
for argument in "$@"; do
    if [ "$argument" = "-mc-environment-sandbox" ]; then sandbox=1; fi
done
if [ "$sandbox" = 0 ] && { [ -e "$script_dir/Builds/Presentation" ] || [ -L "$script_dir/Builds/Presentation" ]; }; then
    exec "$script_dir/run-presentation.sh" modern "$@"
fi
if [ "$sandbox" = 0 ] && [ -x "$script_dir/Builds/CharactersGameplay/run-player.sh" ]; then
    modern_project="$script_dir/Builds/CharactersGameplay"
    export MC_CAMPAIGN_SAVE="${MC_CAMPAIGN_SAVE:-$script_dir/../generated/modern-character-campaign-save.json}"
fi
if [ -e "$modern_project" ] || [ -L "$modern_project" ]; then
    if [ ! -x "$modern_project/run-player.sh" ]; then
        echo "Modern installation is unavailable: $modern_project/run-player.sh" >&2
        exit 1
    fi
    export MC_PLAYER_PROJECT="$modern_project"
fi
exec "$script_dir/run-campaign.sh" -mc-modern "$@"
