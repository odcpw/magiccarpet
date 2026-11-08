#!/bin/sh
set -eu
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
if [ "${1:-}" = "--classic" ]; then
    shift
    exec "$script_dir/Builds/Voxels/MagicCarpetVoxels.x86_64" -force-vulkan -screen-fullscreen 0 "$@"
fi
player_dir="$script_dir/Builds/VoxelsModern"
if [ "${1:-}" = "--connected" ]; then
    shift
    player_dir="$script_dir/Builds/VoxelsConnected"
fi
exec python3 - "$player_dir" "$@" <<'PY'
import hashlib
import json
import os
from pathlib import Path
import sys

try:
    root=Path(sys.argv[1]).resolve(strict=True)
    receipt=json.loads((root/'voxel-install.json').read_text())
    for name,expected in receipt['qualified_files'].items():
        path=root/name
        with path.open('rb') as stream:
            if hashlib.file_digest(stream,'sha256').hexdigest()!=expected:
                raise ValueError(f'Qualified voxel player changed: {path}')
    player=root/'MagicCarpetVoxels.x86_64'
    extra=sys.argv[2:]
    preset='world'
    if extra and extra[0] in ('--goat','--archer'):
        preset=extra.pop(0)[2:]
    arguments=receipt['launch_arguments'] if preset=='world' else receipt['character_arguments'][preset]
    if not isinstance(arguments,list) or not all(isinstance(x,str) for x in arguments):
        raise ValueError('Invalid installed launch arguments')
    if os.environ.get('MC_VOXEL_SAVE'):
        arguments[arguments.index('-mc-runtime-replay-save')+1]=str(Path(os.environ['MC_VOXEL_SAVE']).absolute())
    if os.environ.get('MC_VOXEL_LOAD'):
        arguments[arguments.index('-mc-runtime-replay-load')+1]=str(Path(os.environ['MC_VOXEL_LOAD']).resolve(strict=True))
    options=['-force-vulkan','-screen-fullscreen','0']
    if os.environ.get('WAYLAND_DISPLAY'):
        options+=['-force-wayland']
        os.environ.setdefault('__NV_DISABLE_EXPLICIT_SYNC','1')
    os.execv(str(player),[str(player),*options,*arguments,*extra])
except (OSError,ValueError,KeyError) as error:
    print(f'Voxel sandbox unavailable: {error}. See VOXEL_SANDBOX.md.',file=sys.stderr)
    sys.exit(1)
PY
