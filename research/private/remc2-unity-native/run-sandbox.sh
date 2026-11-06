#!/bin/sh
# A qualified, frozen environment snapshot; campaign saves are never selected.
set -eu
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
exec python3 - "$script_dir" "$@" <<'PY'
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile

project = Path(sys.argv[1])
installed = project / 'Builds/Modern'
if not installed.exists():
    installed = project
profile = installed / 'sandbox-inputs.json'

def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

try:
    data = json.loads(profile.read_text())
    if data['schema'] != 'mc-environment-sandbox/v1':
        raise ValueError('unsupported sandbox input schema')
    replay = Path(data['replay'])
    plugin = installed / 'Builds/Linux/MagicCarpetNative_Data/Plugins/x86_64/libsim_capi.so'
    if digest(replay) != data['replay_sha256'] or digest(plugin) != data['native_sha256']:
        raise ValueError('sandbox snapshot or installed native differs from its qualified inputs')
    if 'building_layout' in data:
        layout = Path(data['building_layout'])
        if digest(layout) != data['building_layout_sha256']:
            raise ValueError('sandbox Level 1 building layout differs from its qualified input')
        os.environ['MC_SANDBOX_BUILDING_LAYOUT'] = str(layout)
    args = [str(project / 'run-modern.sh'), '--load-save', str(replay)]
    if not data['packages']:
        raise ValueError('sandbox requires an installed terrain package')
    for resource in data['packages']:
        for key, option in [('package', '--package'), ('owner', '--owner-cache'), ('presentation', '--presentation-cache')]:
            value = Path(resource[key])
            if not value.is_dir():
                raise ValueError(f'missing sandbox {key}: {value}')
            args.extend((option, str(value)))
    args.extend(('--loading-cache', data['loading'], '--menu-cache', data['menu']))
    destination = Path(tempfile.mkdtemp(prefix='magiccarpet-sandbox.')) / 'session.json'
    args.extend(('--save', str(destination), '-mc-environment-sandbox', *sys.argv[2:]))
except (OSError, ValueError, KeyError, TypeError) as error:
    print(f'Sandbox is not qualified: {error}. Install matching sandbox inputs at {profile}.', file=sys.stderr)
    sys.exit(1)
os.execv(args[0], args)
PY
