#!/bin/sh
set -eu
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
player="$script_dir/Builds/Characters/MagicCarpetCharacters.x86_64"
if [ ! -x "$player" ]; then
    echo "Character player is not installed at $player. See ../character-pipeline/README.md." >&2
    exit 1
fi
exec "$player" -screen-fullscreen 0 "$@"
