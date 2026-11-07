#!/bin/sh
set -eu
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ "${1:-}" = "--preview" ]; then
    shift
    player="$script_dir/Builds/Characters/MagicCarpetCharacters.x86_64"
    if [ ! -x "$player" ]; then
        echo "Character inspection player is unavailable: $player" >&2
        exit 1
    fi
    exec "$player" -screen-fullscreen 0 "$@"
fi
gameplay="$script_dir/Builds/CharactersGameplay"
if [ ! -x "$gameplay/run-player.sh" ]; then
    echo "Live character player is not installed at $gameplay. See ../character-pipeline/README.md." >&2
    exit 1
fi
export MC_PLAYER_PROJECT="$gameplay"
export MC_CAMPAIGN_SAVE="${MC_CAMPAIGN_SAVE:-$script_dir/../generated/modern-character-campaign-save.json}"
if [ "$#" = 0 ]; then set -- --new-game; fi
exec "$script_dir/run-campaign.sh" -mc-modern "$@"
