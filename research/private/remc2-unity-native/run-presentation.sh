#!/bin/sh
set -eu
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
exec python3 - "$script_dir" "$@" <<'PY'
import hashlib
import json
import os
from pathlib import Path
import sys

def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

try:
    client = Path(sys.argv[1])
    mode = sys.argv[2]
    if mode not in ('modern', 'voxel', 'classic'):
        raise ValueError('Expected modern, voxel or classic presentation')
    root = Path(os.environ.get('MC_PRESENTATION_PLAYER', client / 'Builds/Presentation')).resolve(strict=True)
    receipt = json.loads((root / 'presentation-install.json').read_text())
    if receipt['schema'] != 'mc-presentation-install/v1':
        raise ValueError('Unsupported presentation installation')
    for name, expected in receipt['qualified_files'].items():
        path = (root / name).resolve(strict=True)
        if not path.is_relative_to(root) or digest(path) != expected:
            raise ValueError(f'Qualified presentation payload changed: {name}')
    layout = receipt['owner_building_layout']
    layout_path = Path(layout['path']).resolve(strict=True)
    if digest(layout_path) != layout['sha256']:
        raise ValueError('Owner building layout changed')
    os.environ['MC_SANDBOX_BUILDING_LAYOUT'] = str(layout_path)
    player = (root / receipt['player_executable']).resolve(strict=True)
    if not player.is_relative_to(root) or receipt['player_executable'] not in receipt['qualified_files']:
        raise ValueError('Player executable is outside the qualified installation')
    extra = sys.argv[3:]
    preset = extra.pop(0)[2:] if extra and extra[0] in ('--goat', '--archer') else 'world'
    base_arguments = receipt['launch_arguments'][preset]
    mode_arguments = receipt['mode_arguments'][mode]
    if not all(isinstance(items, list) and all(isinstance(item, str) for item in items)
               for items in (base_arguments, mode_arguments)):
        raise ValueError('Invalid presentation launch arguments')
    arguments = base_arguments + mode_arguments
    for path, expected in receipt['qualified_sidecars'].items():
        if digest(Path(path).resolve(strict=True)) != expected:
            raise ValueError(f'Qualified presentation sidecar changed: {path}')
    load = os.environ.get('MC_VOXEL_LOAD') if mode == 'voxel' else os.environ.get('MC_CAMPAIGN_LOAD')
    save = os.environ.get('MC_VOXEL_SAVE') if mode == 'voxel' else os.environ.get('MC_CAMPAIGN_SAVE')
    if load:
        arguments[arguments.index('-mc-runtime-replay-load') + 1] = str(Path(load).resolve(strict=True))
    if save:
        arguments[arguments.index('-mc-runtime-replay-save') + 1] = str(Path(save).absolute())
    options = ['-force-vulkan', '-screen-fullscreen', '0']
    if os.environ.get('WAYLAND_DISPLAY'):
        options.append('-force-wayland')
        os.environ.setdefault('__NV_DISABLE_EXPLICIT_SYNC', '1')
    os.execv(str(player), [str(player), *options, *arguments, *extra])
except (OSError, ValueError, KeyError, IndexError, TypeError) as error:
    print(f'Presentation client unavailable: {error}. See PRESENTATION.md.', file=sys.stderr)
    sys.exit(1)
PY
