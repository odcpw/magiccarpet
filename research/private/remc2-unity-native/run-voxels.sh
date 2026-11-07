#!/bin/sh
set -eu
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
player="$script_dir/Builds/Voxels/MagicCarpetVoxels.x86_64"
if [ ! -x "$player" ]; then
    echo "Voxel sandbox is not installed at $player. See VOXEL_SANDBOX.md." >&2
    exit 1
fi
exec "$player" -force-vulkan -screen-fullscreen 0 "$@"
